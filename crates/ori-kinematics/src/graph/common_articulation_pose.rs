use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    CandidateFaceTransform, CanonicalMaterialEdgeBlockDecompositionV1, ClosedMaterialHingeGraphPose,
};
use crate::{
    CanonicalHingeAngles, MaterialHingeGraphGeometry, RigidTransform,
    tree::MaterialHingeGraphInstanceV1,
};

pub const COMMON_ARTICULATION_POSE_MODEL_ID_V1: &str = "common_articulation_pose_authority_v1";
pub const COMMON_ARTICULATION_POSE_MIN_BLOCKS_V1: usize = 2;
pub const COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1: usize = 10;
/// Domain identifier for the separately typed, non-authorizing pose extension.
pub const COMMON_ARTICULATION_POSE_EXTENSION_MODEL_ID_V1: &str =
    "common_articulation_pose_extension_authority_v1";
/// Minimum actual block count admitted by the pose extension.
pub const COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1: usize = 11;
/// Hard maximum configured cap admitted by the pose extension.
pub const COMMON_ARTICULATION_POSE_EXTENSION_MAX_BLOCKS_V1: usize = 32;
/// Hard maximum parent-face count admitted only by the pose extension.
pub const COMMON_ARTICULATION_POSE_EXTENSION_MAX_FACES_V1: usize = 257;
/// Hard maximum parent-hinge count admitted only by the pose extension.
pub const COMMON_ARTICULATION_POSE_EXTENSION_MAX_HINGES_V1: usize = 384;

const COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1: usize = 8;
const COMMON_ARTICULATION_POSE_MAX_FACES_V1: usize = 256;
const COMMON_ARTICULATION_POSE_MAX_HINGES_V1: usize = 256;
const COMMON_ARTICULATION_POSE_MAX_WORK_V1: usize = 65_536;
const COMMON_ARTICULATION_POSE_MAX_RETAINED_BYTES_V1: usize = 4 * 1024 * 1024;
const AUTHORITY_BASE_BYTES_V1: usize = 512;
const BLOCK_RECORD_BYTES_V1: usize = 192;
const FACE_TRANSFORM_RECORD_BYTES_V1: usize = 128;
const HINGE_ANGLE_RECORD_BYTES_V1: usize = 32;
const ARTICULATION_FACE_RECORD_BYTES_V1: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationPoseLimitsV1 {
    pub max_blocks: usize,
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_work: usize,
    pub max_retained_bytes: usize,
}

impl Default for CommonArticulationPoseLimitsV1 {
    fn default() -> Self {
        Self {
            max_blocks: COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1,
            max_faces: COMMON_ARTICULATION_POSE_MAX_FACES_V1,
            max_hinges: COMMON_ARTICULATION_POSE_MAX_HINGES_V1,
            max_work: COMMON_ARTICULATION_POSE_MAX_WORK_V1,
            max_retained_bytes: COMMON_ARTICULATION_POSE_MAX_RETAINED_BYTES_V1,
        }
    }
}

/// Explicit resource envelope for the separately typed 11..=32 pose extension.
///
/// This type has no default so callers must state the inclusive block cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationPoseExtensionLimitsV1 {
    /// Inclusive configured block cap; valid values are 11 through 32.
    pub max_blocks: usize,
    /// Maximum live parent faces retained and revalidated.
    pub max_faces: usize,
    /// Maximum live parent hinges retained and revalidated.
    pub max_hinges: usize,
    /// Maximum deterministic logical-work estimate.
    pub max_work: usize,
    /// Maximum retained authority byte upper bound.
    pub max_retained_bytes: usize,
}

impl CommonArticulationPoseExtensionLimitsV1 {
    /// Builds the standard hard resource envelope for one valid configured cap.
    #[must_use]
    pub const fn with_max_blocks_v1(max_blocks: usize) -> Option<Self> {
        if max_blocks < COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1
            || max_blocks > COMMON_ARTICULATION_POSE_EXTENSION_MAX_BLOCKS_V1
        {
            return None;
        }
        Some(Self {
            max_blocks,
            max_faces: COMMON_ARTICULATION_POSE_EXTENSION_MAX_FACES_V1,
            max_hinges: COMMON_ARTICULATION_POSE_EXTENSION_MAX_HINGES_V1,
            max_work: COMMON_ARTICULATION_POSE_MAX_WORK_V1,
            max_retained_bytes: COMMON_ARTICULATION_POSE_MAX_RETAINED_BYTES_V1,
        })
    }

    fn as_internal_limits_v1(self) -> CommonArticulationPoseLimitsV1 {
        CommonArticulationPoseLimitsV1 {
            max_blocks: self.max_blocks,
            max_faces: self.max_faces,
            max_hinges: self.max_hinges,
            max_work: self.max_work,
            max_retained_bytes: self.max_retained_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationPoseStopV1 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationPoseErrorV1 {
    #[error("the common-articulation-pose input is malformed")]
    InvalidInput,
    #[error("the common-articulation-pose proof exceeds its explicit resource limits")]
    ResourceLimit,
    #[error("the closed pose was not issued by the live parent geometry")]
    PoseIssuerMismatch,
    #[error("the canonical decomposition was not issued by the live parent geometry")]
    DecompositionIssuerMismatch,
    #[error("the canonical decomposition is not an exact block restriction of the parent")]
    InvalidDecomposition,
    #[error("the closed parent pose is incomplete or inconsistent")]
    IncompleteParentPose,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the operation deadline elapsed")]
    DeadlineExceeded,
    #[error("the retained authority does not match the supplied live inputs")]
    IssuerMismatch,
}

#[derive(Clone, Copy)]
pub struct CommonArticulationPoseInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub paper_thickness_mm: f64,
    pub limits: CommonArticulationPoseLimitsV1,
}

/// Live inputs for the separately typed, non-authorizing pose extension.
#[derive(Clone, Copy)]
pub struct CommonArticulationPoseExtensionInputV1<'a> {
    /// Live parent material-hinge geometry.
    pub geometry: &'a MaterialHingeGraphGeometry,
    /// Closed pose issued by the exact live geometry instance.
    pub pose: &'a ClosedMaterialHingeGraphPose,
    /// Canonical block decomposition issued by the exact live geometry.
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    /// Strictly positive finite paper thickness bound into the authority.
    pub paper_thickness_mm: f64,
    /// Explicit extension admission and resource envelope.
    pub limits: CommonArticulationPoseExtensionLimitsV1,
}

impl<'a> CommonArticulationPoseExtensionInputV1<'a> {
    fn as_internal_input_v1(self) -> CommonArticulationPoseInputV1<'a> {
        CommonArticulationPoseInputV1 {
            geometry: self.geometry,
            pose: self.pose,
            decomposition: self.decomposition,
            paper_thickness_mm: self.paper_thickness_mm,
            limits: self.limits.as_internal_limits_v1(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommonArticulationPoseProofScopeV1 {
    LegacyHardTen,
    Extension { configured_max_blocks: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationHingeAngleBitsV1 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

impl CommonArticulationHingeAngleBitsV1 {
    #[must_use]
    pub const fn edge(self) -> EdgeId {
        self.edge
    }

    #[must_use]
    pub const fn angle_degrees_bits(self) -> u64 {
        self.angle_degrees_bits
    }
}

#[derive(Debug)]
struct CommonArticulationPoseBlockRestrictionV1 {
    geometry_issuer: MaterialHingeGraphInstanceV1,
    faces: Vec<FaceId>,
    face_transforms: Vec<CandidateFaceTransform>,
    hinge_angles: Vec<CommonArticulationHingeAngleBitsV1>,
    articulation_faces: Vec<FaceId>,
}

pub struct CommonArticulationPoseBlockRestrictionRefV1<'a> {
    block: &'a CommonArticulationPoseBlockRestrictionV1,
}

impl CommonArticulationPoseBlockRestrictionRefV1<'_> {
    #[must_use]
    pub fn is_for_geometry(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.block.geometry_issuer.matches(geometry)
            && self.block.faces == geometry.face_ids()
            && self.block.hinge_angles.len() == geometry.hinges().len()
            && self
                .block
                .hinge_angles
                .iter()
                .zip(geometry.hinges())
                .all(|(angle, hinge)| angle.edge == hinge.edge())
    }

    #[must_use]
    pub fn face_ids(&self) -> &[FaceId] {
        &self.block.faces
    }

    #[must_use]
    pub fn face_transforms(&self) -> &[CandidateFaceTransform] {
        &self.block.face_transforms
    }

    #[must_use]
    pub fn hinge_angles(&self) -> &[CommonArticulationHingeAngleBitsV1] {
        &self.block.hinge_angles
    }

    #[must_use]
    pub fn articulation_faces(&self) -> &[FaceId] {
        &self.block.articulation_faces
    }
}

/// Sealed proof that every canonical block is one bit-exact restriction of a
/// single live, closed parent pose.
///
/// The authority is deliberately neither cloneable nor serializable. It is a
/// pose-provenance prerequisite only and grants no motion, collision, viewer,
/// Apply, or project-mutation authority.
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationPoseAuthorityV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationPoseAuthorityV1>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationPoseAuthorityV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationPoseAuthorityV1>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationPoseAuthorityV1 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    issuer_pose: Arc<()>,
    fixed_face: FaceId,
    decomposition_limits: super::CanonicalEdgeBlockLimitsV1,
    paper_thickness_bits: u64,
    limits: CommonArticulationPoseLimitsV1,
    blocks: Vec<CommonArticulationPoseBlockRestrictionV1>,
    articulation_faces: Vec<FaceId>,
    logical_work: usize,
    retained_bytes: usize,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationPoseAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_POSE_MODEL_ID_V1
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.blocks.len()
    }

    #[must_use]
    pub const fn fixed_face_v1(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn articulation_faces_v1(&self) -> &[FaceId] {
        &self.articulation_faces
    }

    #[must_use]
    pub fn block_v1(
        &self,
        index: usize,
    ) -> Option<CommonArticulationPoseBlockRestrictionRefV1<'_>> {
        self.blocks
            .get(index)
            .map(|block| CommonArticulationPoseBlockRestrictionRefV1 { block })
    }

    #[must_use]
    pub const fn paper_thickness_mm_v1(&self) -> f64 {
        f64::from_bits(self.paper_thickness_bits)
    }

    #[must_use]
    pub const fn logical_work_v1(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn retained_bytes_upper_bound_v1(&self) -> usize {
        self.retained_bytes
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub fn revalidate_v1(
        &self,
        input: CommonArticulationPoseInputV1<'_>,
    ) -> Result<(), CommonArticulationPoseErrorV1> {
        self.revalidate_with_checkpoint_v1(input, || Ok(()))
    }

    /// Reproves the live input and compares every retained binding while
    /// cooperatively observing cancellation or deadline checkpoints.
    pub fn revalidate_with_checkpoint_v1(
        &self,
        input: CommonArticulationPoseInputV1<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
    ) -> Result<(), CommonArticulationPoseErrorV1> {
        let candidate =
            prove_common_articulation_pose_authority_with_checkpoint_v1(input, &mut checkpoint)?;
        common_articulation_checkpoint_v1(&mut checkpoint)?;
        if self.matches_candidate_with_checkpoint_v1(&candidate, &mut checkpoint)? {
            common_articulation_checkpoint_v1(&mut checkpoint)?;
            Ok(())
        } else {
            Err(CommonArticulationPoseErrorV1::IssuerMismatch)
        }
    }

    fn matches_candidate_with_checkpoint_v1(
        &self,
        candidate: &Self,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
    ) -> Result<bool, CommonArticulationPoseErrorV1> {
        common_articulation_checkpoint_v1(checkpoint)?;
        if self.issuer_geometry != candidate.issuer_geometry
            || !Arc::ptr_eq(&self.issuer_pose, &candidate.issuer_pose)
            || self.fixed_face != candidate.fixed_face
            || self.decomposition_limits != candidate.decomposition_limits
            || self.paper_thickness_bits != candidate.paper_thickness_bits
            || self.limits != candidate.limits
            || self.logical_work != candidate.logical_work
            || self.retained_bytes != candidate.retained_bytes
            || self.binding_fingerprint != candidate.binding_fingerprint
            || self.blocks.len() != candidate.blocks.len()
        {
            return Ok(false);
        }
        if !candidate_slice_equal_with_checkpoint_v1(
            &self.articulation_faces,
            &candidate.articulation_faces,
            checkpoint,
        )? {
            return Ok(false);
        }
        for (expected, actual) in self.blocks.iter().zip(&candidate.blocks) {
            common_articulation_checkpoint_v1(checkpoint)?;
            if expected.geometry_issuer != actual.geometry_issuer
                || !candidate_slice_equal_with_checkpoint_v1(
                    &expected.faces,
                    &actual.faces,
                    checkpoint,
                )?
                || !candidate_face_transforms_bit_equal_with_checkpoint_v1(
                    &expected.face_transforms,
                    &actual.face_transforms,
                    checkpoint,
                )?
                || !candidate_slice_equal_with_checkpoint_v1(
                    &expected.hinge_angles,
                    &actual.hinge_angles,
                    checkpoint,
                )?
                || !candidate_slice_equal_with_checkpoint_v1(
                    &expected.articulation_faces,
                    &actual.articulation_faces,
                    checkpoint,
                )?
            {
                return Ok(false);
            }
        }
        Ok(true)
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
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

/// Sealed pose provenance for 11 through an explicitly configured cap.
///
/// This distinct authority type cannot be supplied to the legacy clearance,
/// staged/final, desktop, Apply, or viewer paths. It remains non-authorizing.
///
/// ```compile_fail
/// use ori_kinematics::{
///     CommonArticulationPoseAuthorityV1, CommonArticulationPoseExtensionAuthorityV1,
/// };
///
/// fn legacy_pose(_: CommonArticulationPoseAuthorityV1) {}
/// fn cannot_route(extension: CommonArticulationPoseExtensionAuthorityV1) {
///     legacy_pose(extension);
/// }
/// ```
#[derive(Debug)]
pub struct CommonArticulationPoseExtensionAuthorityV1 {
    inner: CommonArticulationPoseAuthorityV1,
}

impl CommonArticulationPoseExtensionAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_POSE_EXTENSION_MODEL_ID_V1
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.inner.block_count_v1()
    }

    #[must_use]
    pub const fn configured_max_blocks_v1(&self) -> usize {
        self.inner.limits.max_blocks
    }

    #[must_use]
    pub const fn fixed_face_v1(&self) -> FaceId {
        self.inner.fixed_face_v1()
    }

    #[must_use]
    pub fn articulation_faces_v1(&self) -> &[FaceId] {
        self.inner.articulation_faces_v1()
    }

    #[must_use]
    pub fn block_v1(
        &self,
        index: usize,
    ) -> Option<CommonArticulationPoseBlockRestrictionRefV1<'_>> {
        self.inner.block_v1(index)
    }

    #[must_use]
    pub const fn paper_thickness_mm_v1(&self) -> f64 {
        self.inner.paper_thickness_mm_v1()
    }

    #[must_use]
    pub const fn logical_work_v1(&self) -> usize {
        self.inner.logical_work_v1()
    }

    #[must_use]
    pub const fn retained_bytes_upper_bound_v1(&self) -> usize {
        self.inner.retained_bytes_upper_bound_v1()
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.inner.binding_fingerprint_v1()
    }

    pub fn revalidate_v1(
        &self,
        input: CommonArticulationPoseExtensionInputV1<'_>,
    ) -> Result<(), CommonArticulationPoseErrorV1> {
        self.revalidate_with_checkpoint_v1(input, || Ok(()))
    }

    /// Reproves the extension input and observes cooperative stop checkpoints.
    pub fn revalidate_with_checkpoint_v1(
        &self,
        input: CommonArticulationPoseExtensionInputV1<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
    ) -> Result<(), CommonArticulationPoseErrorV1> {
        let candidate = prove_common_articulation_pose_extension_authority_with_checkpoint_v1(
            input,
            &mut checkpoint,
        )?;
        common_articulation_checkpoint_v1(&mut checkpoint)?;
        if self
            .inner
            .matches_candidate_with_checkpoint_v1(&candidate.inner, &mut checkpoint)?
        {
            common_articulation_checkpoint_v1(&mut checkpoint)?;
            Ok(())
        } else {
            Err(CommonArticulationPoseErrorV1::IssuerMismatch)
        }
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
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

pub fn prove_common_articulation_pose_authority_v1(
    input: CommonArticulationPoseInputV1<'_>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    prove_common_articulation_pose_authority_with_checkpoint_v1(input, || Ok(()))
}

pub fn prove_common_articulation_pose_authority_with_checkpoint_v1(
    input: CommonArticulationPoseInputV1<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    prove_common_articulation_pose_authority_in_scope_with_checkpoint_v1(
        input,
        CommonArticulationPoseProofScopeV1::LegacyHardTen,
        &mut checkpoint,
    )
}

/// Proves the separately typed, non-authorizing 11..=configured-cap pose scope.
pub fn prove_common_articulation_pose_extension_authority_v1(
    input: CommonArticulationPoseExtensionInputV1<'_>,
) -> Result<CommonArticulationPoseExtensionAuthorityV1, CommonArticulationPoseErrorV1> {
    prove_common_articulation_pose_extension_authority_with_checkpoint_v1(input, || Ok(()))
}

/// Proves the pose extension while observing cooperative stop checkpoints.
pub fn prove_common_articulation_pose_extension_authority_with_checkpoint_v1(
    input: CommonArticulationPoseExtensionInputV1<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<CommonArticulationPoseExtensionAuthorityV1, CommonArticulationPoseErrorV1> {
    let configured_max_blocks = input.limits.max_blocks;
    let inner = prove_common_articulation_pose_authority_in_scope_with_checkpoint_v1(
        input.as_internal_input_v1(),
        CommonArticulationPoseProofScopeV1::Extension {
            configured_max_blocks,
        },
        &mut checkpoint,
    )?;
    Ok(CommonArticulationPoseExtensionAuthorityV1 { inner })
}

fn prove_common_articulation_pose_authority_in_scope_with_checkpoint_v1(
    input: CommonArticulationPoseInputV1<'_>,
    scope: CommonArticulationPoseProofScopeV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    common_articulation_checkpoint_v1(checkpoint)?;
    match scope {
        CommonArticulationPoseProofScopeV1::LegacyHardTen => validate_limits_v1(input.limits)?,
        CommonArticulationPoseProofScopeV1::Extension {
            configured_max_blocks,
        } => validate_extension_limits_v1(input.limits, configured_max_blocks)?,
    }
    if !input.paper_thickness_mm.is_finite() || input.paper_thickness_mm <= 0.0 {
        return Err(CommonArticulationPoseErrorV1::InvalidInput);
    }
    if !input.pose.is_for_geometry(input.geometry) {
        return Err(CommonArticulationPoseErrorV1::PoseIssuerMismatch);
    }
    if !input.decomposition.is_for_geometry(input.geometry) {
        return Err(CommonArticulationPoseErrorV1::DecompositionIssuerMismatch);
    }

    let block_count = input.decomposition.blocks().len();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    let block_count_in_scope = match scope {
        CommonArticulationPoseProofScopeV1::LegacyHardTen => {
            (COMMON_ARTICULATION_POSE_MIN_BLOCKS_V1..=COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1)
                .contains(&block_count)
        }
        CommonArticulationPoseProofScopeV1::Extension {
            configured_max_blocks,
        } => (COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1..=configured_max_blocks)
            .contains(&block_count),
    };
    if !block_count_in_scope
        || block_count > input.limits.max_blocks
        || face_count == 0
        || face_count > input.limits.max_faces
        || hinge_count == 0
        || hinge_count > input.limits.max_hinges
    {
        return Err(CommonArticulationPoseErrorV1::ResourceLimit);
    }

    let mut submitted_faces = 0usize;
    let mut submitted_hinges = 0usize;
    for block in input.decomposition.blocks() {
        common_articulation_checkpoint_v1(checkpoint)?;
        submitted_faces = submitted_faces
            .checked_add(block.geometry().face_ids().len())
            .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
        submitted_hinges = submitted_hinges
            .checked_add(block.geometry().hinges().len())
            .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
    }
    let block_pairs = block_count
        .checked_mul(block_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
    let logical_work = 16usize
        .checked_add(
            face_count
                .checked_mul(8)
                .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?,
        )
        .and_then(|value| value.checked_add(hinge_count.checked_mul(12)?))
        .and_then(|value| value.checked_add(block_count.checked_mul(16)?))
        .and_then(|value| value.checked_add(submitted_faces.checked_mul(8)?))
        .and_then(|value| value.checked_add(submitted_hinges.checked_mul(12)?))
        .and_then(|value| value.checked_add(block_pairs.checked_mul(8)?))
        .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
    let retained_bytes = retained_bytes_v1(
        block_count,
        submitted_faces,
        submitted_hinges,
        input.decomposition.articulation_faces().len(),
    )?;
    if logical_work > input.limits.max_work || retained_bytes > input.limits.max_retained_bytes {
        return Err(CommonArticulationPoseErrorV1::ResourceLimit);
    }

    common_articulation_checkpoint_v1(checkpoint)?;
    validate_parent_pose_v1(input.geometry, input.pose, checkpoint)?;
    validate_decomposition_v1(input.geometry, input.decomposition, checkpoint)?;

    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(block_count)
        .map_err(|_| CommonArticulationPoseErrorV1::ResourceLimit)?;
    for block in input.decomposition.blocks() {
        common_articulation_checkpoint_v1(checkpoint)?;
        let mut face_transforms = Vec::new();
        face_transforms
            .try_reserve_exact(block.geometry().face_ids().len())
            .map_err(|_| CommonArticulationPoseErrorV1::ResourceLimit)?;
        for face in block.geometry().face_ids().iter().copied() {
            common_articulation_checkpoint_v1(checkpoint)?;
            let transform = input
                .pose
                .face_transform(face)
                .ok_or(CommonArticulationPoseErrorV1::IncompleteParentPose)?;
            face_transforms.push(CandidateFaceTransform::new(face, transform));
        }

        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(block.geometry().hinges().len())
            .map_err(|_| CommonArticulationPoseErrorV1::ResourceLimit)?;
        for hinge in block.geometry().hinges() {
            common_articulation_checkpoint_v1(checkpoint)?;
            let angle_degrees_bits = angle_bits_v1(input.pose.hinge_angles(), hinge.edge())
                .ok_or(CommonArticulationPoseErrorV1::IncompleteParentPose)?;
            hinge_angles.push(CommonArticulationHingeAngleBitsV1 {
                edge: hinge.edge(),
                angle_degrees_bits,
            });
        }
        hinge_angles.sort_unstable_by_key(|angle| angle.edge.canonical_bytes());

        let mut articulation_faces = Vec::new();
        for face in block.geometry().face_ids().iter().copied() {
            common_articulation_checkpoint_v1(checkpoint)?;
            if input
                .decomposition
                .articulation_faces()
                .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
                .is_ok()
            {
                articulation_faces.push(face);
            }
        }
        articulation_faces.sort_unstable_by_key(FaceId::canonical_bytes);
        blocks.push(CommonArticulationPoseBlockRestrictionV1 {
            geometry_issuer: block.geometry().instance_anchor_v1(),
            faces: block.geometry().face_ids().to_vec(),
            face_transforms,
            hinge_angles,
            articulation_faces,
        });
    }

    common_articulation_checkpoint_v1(checkpoint)?;
    let binding_fingerprint = match scope {
        CommonArticulationPoseProofScopeV1::LegacyHardTen => {
            binding_fingerprint_with_checkpoint_v1(
                input.geometry,
                input.pose,
                &blocks,
                input.decomposition.articulation_faces(),
                input.paper_thickness_mm.to_bits(),
                checkpoint,
            )?
        }
        CommonArticulationPoseProofScopeV1::Extension {
            configured_max_blocks,
        } => extension_binding_fingerprint_with_checkpoint_v1(
            input.geometry,
            input.pose,
            &blocks,
            input.decomposition.articulation_faces(),
            input.paper_thickness_mm.to_bits(),
            configured_max_blocks,
            checkpoint,
        )?,
    };
    common_articulation_checkpoint_v1(checkpoint)?;
    Ok(CommonArticulationPoseAuthorityV1 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        issuer_pose: Arc::clone(&input.pose.instance),
        fixed_face: input.pose.fixed_face(),
        decomposition_limits: input.decomposition.limits(),
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        limits: input.limits,
        blocks,
        articulation_faces: input.decomposition.articulation_faces().to_vec(),
        logical_work,
        retained_bytes,
        binding_fingerprint,
    })
}

fn validate_limits_v1(
    limits: CommonArticulationPoseLimitsV1,
) -> Result<(), CommonArticulationPoseErrorV1> {
    if limits.max_blocks > COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1
        || limits.max_faces > COMMON_ARTICULATION_POSE_MAX_FACES_V1
        || limits.max_hinges > COMMON_ARTICULATION_POSE_MAX_HINGES_V1
        || limits.max_work > COMMON_ARTICULATION_POSE_MAX_WORK_V1
        || limits.max_retained_bytes > COMMON_ARTICULATION_POSE_MAX_RETAINED_BYTES_V1
    {
        return Err(CommonArticulationPoseErrorV1::ResourceLimit);
    }
    Ok(())
}

fn validate_extension_limits_v1(
    limits: CommonArticulationPoseLimitsV1,
    configured_max_blocks: usize,
) -> Result<(), CommonArticulationPoseErrorV1> {
    if limits.max_blocks != configured_max_blocks
        || !(COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1
            ..=COMMON_ARTICULATION_POSE_EXTENSION_MAX_BLOCKS_V1)
            .contains(&configured_max_blocks)
        || limits.max_faces > COMMON_ARTICULATION_POSE_EXTENSION_MAX_FACES_V1
        || limits.max_hinges > COMMON_ARTICULATION_POSE_EXTENSION_MAX_HINGES_V1
        || limits.max_work > COMMON_ARTICULATION_POSE_MAX_WORK_V1
        || limits.max_retained_bytes > COMMON_ARTICULATION_POSE_MAX_RETAINED_BYTES_V1
    {
        return Err(CommonArticulationPoseErrorV1::ResourceLimit);
    }
    Ok(())
}

fn validate_parent_pose_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<(), CommonArticulationPoseErrorV1> {
    if !geometry.face_ids().contains(&pose.fixed_face())
        || pose.transforms().len() != geometry.face_ids().len()
        || pose.hinge_angles().as_slice().len() != geometry.hinges().len()
        || pose.closure_certificate().checked_hinges().len() != geometry.hinges().len()
        || !pose
            .closure_certificate()
            .maximum_axis_point_error()
            .is_finite()
        || !pose
            .closure_certificate()
            .maximum_relative_transform_error()
            .is_finite()
    {
        return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
    }
    for faces in geometry.face_ids().windows(2) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if faces[0].canonical_bytes() >= faces[1].canonical_bytes() {
            return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
        }
    }
    for (expected, transform) in geometry.face_ids().iter().zip(pose.transforms()) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if transform.face() != *expected || !rigid_transform_is_finite_v1(transform.transform()) {
            return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
        }
    }

    let mut canonical_edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    for edges in canonical_edges.windows(2) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if edges[0] == edges[1] {
            return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
        }
    }
    for (edge, angle) in canonical_edges.iter().zip(pose.hinge_angles().as_slice()) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if *edge != angle.edge()
            || !angle.angle_degrees().is_finite()
            || !(0.0..=180.0).contains(&angle.angle_degrees())
        {
            return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
        }
    }
    common_articulation_checkpoint_v1(checkpoint)?;
    if canonical_edges != pose.closure_certificate().checked_hinges() {
        return Err(CommonArticulationPoseErrorV1::IncompleteParentPose);
    }
    Ok(())
}

fn validate_decomposition_v1(
    geometry: &MaterialHingeGraphGeometry,
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<(), CommonArticulationPoseErrorV1> {
    for faces in decomposition.articulation_faces().windows(2) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if faces[0].canonical_bytes() >= faces[1].canonical_bytes() {
            return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
        }
    }

    let mut union_faces = Vec::new();
    let mut union_hinges = Vec::new();
    let mut incidence = HashMap::<FaceId, usize>::new();
    let mut prior_key = None;
    for block in decomposition.blocks() {
        common_articulation_checkpoint_v1(checkpoint)?;
        let faces = block.geometry().face_ids();
        let hinges = block.geometry().hinges();
        if faces.len() < 2 || hinges.is_empty() {
            return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
        }
        for pair in faces.windows(2) {
            common_articulation_checkpoint_v1(checkpoint)?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
            }
        }
        let mut block_edges = Vec::new();
        for hinge in hinges {
            common_articulation_checkpoint_v1(checkpoint)?;
            block_edges.push(hinge.edge());
        }
        block_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        for pair in block_edges.windows(2) {
            common_articulation_checkpoint_v1(checkpoint)?;
            if pair[0] == pair[1] {
                return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
            }
        }
        let key = (faces[0].canonical_bytes(), block_edges[0].canonical_bytes());
        if prior_key.is_some_and(|prior| prior >= key) {
            return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
        }
        prior_key = Some(key);

        let mut audit_edges = Vec::new();
        for edge in block
            .audit()
            .spanning_hinges()
            .iter()
            .chain(block.audit().closure_hinges())
            .copied()
        {
            common_articulation_checkpoint_v1(checkpoint)?;
            audit_edges.push(edge);
        }
        audit_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if block.audit().faces() != faces || audit_edges != block_edges {
            return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
        }
        for face in faces.iter().copied() {
            common_articulation_checkpoint_v1(checkpoint)?;
            if !geometry.face_ids().contains(&face) {
                return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
            }
            union_faces.push(face);
            *incidence.entry(face).or_default() += 1;
        }
        for hinge in hinges {
            common_articulation_checkpoint_v1(checkpoint)?;
            let mut parent_hinge = None;
            for candidate in geometry.hinges() {
                common_articulation_checkpoint_v1(checkpoint)?;
                if candidate.edge() == hinge.edge() {
                    parent_hinge = Some(candidate);
                    break;
                }
            }
            let Some(parent_hinge) = parent_hinge else {
                return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
            };
            if parent_hinge != hinge {
                return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
            }
            union_hinges.push(hinge.edge());
        }
    }

    union_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    union_faces.dedup();
    union_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut parent_hinges = Vec::new();
    for hinge in geometry.hinges() {
        common_articulation_checkpoint_v1(checkpoint)?;
        parent_hinges.push(hinge.edge());
    }
    parent_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut articulation_faces = Vec::new();
    for (face, count) in incidence {
        common_articulation_checkpoint_v1(checkpoint)?;
        if count > 1 {
            articulation_faces.push(face);
        }
    }
    articulation_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    for pair in union_hinges.windows(2) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if pair[0] == pair[1] {
            return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
        }
    }
    if union_faces != geometry.face_ids()
        || union_hinges != parent_hinges
        || articulation_faces != decomposition.articulation_faces()
    {
        return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
    }
    if !block_articulation_incidence_is_tree_with_checkpoint_v1(decomposition, checkpoint)? {
        return Err(CommonArticulationPoseErrorV1::InvalidDecomposition);
    }
    Ok(())
}

/// Validates the canonical block-cut incidence graph, rather than the
/// pairwise block-intersection projection.
///
/// A single articulation face may legitimately belong to three or more
/// blocks.  Its incidence graph is one star, while the pairwise projection is
/// a clique and therefore must not be mistaken for a cycle.
fn block_articulation_incidence_is_tree_with_checkpoint_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<bool, CommonArticulationPoseErrorV1> {
    let blocks = decomposition.blocks();
    if blocks.len() < COMMON_ARTICULATION_POSE_MIN_BLOCKS_V1 {
        return Ok(false);
    }
    let mut occurrences = Vec::new();
    for (block_index, block) in blocks.iter().enumerate() {
        common_articulation_checkpoint_v1(checkpoint)?;
        for face in block.geometry().face_ids().iter().copied() {
            common_articulation_checkpoint_v1(checkpoint)?;
            occurrences.push((face, block_index));
        }
    }
    occurrences.sort_unstable_by_key(|(face, block)| (face.canonical_bytes(), *block));

    let mut articulation_memberships = Vec::new();
    let mut cursor = 0usize;
    while cursor < occurrences.len() {
        common_articulation_checkpoint_v1(checkpoint)?;
        let face = occurrences[cursor].0;
        let mut end = cursor + 1;
        while end < occurrences.len() && occurrences[end].0 == face {
            common_articulation_checkpoint_v1(checkpoint)?;
            end += 1;
        }
        if end - cursor > 1 {
            let mut memberships = Vec::new();
            for (_, block_index) in &occurrences[cursor..end] {
                common_articulation_checkpoint_v1(checkpoint)?;
                if memberships.last() == Some(block_index) {
                    return Ok(false);
                }
                memberships.push(*block_index);
            }
            articulation_memberships.push(memberships);
        }
        cursor = end;
    }
    if articulation_memberships.is_empty() {
        return Ok(false);
    }

    let node_count = blocks
        .len()
        .checked_add(articulation_memberships.len())
        .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
    let edge_count = articulation_memberships
        .iter()
        .try_fold(0usize, |sum, memberships| {
            sum.checked_add(memberships.len())
        })
        .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?;
    if edge_count != node_count.saturating_sub(1) {
        return Ok(false);
    }
    let mut adjacency = vec![Vec::new(); node_count];
    for (articulation_index, memberships) in articulation_memberships.iter().enumerate() {
        common_articulation_checkpoint_v1(checkpoint)?;
        let articulation_node = blocks.len() + articulation_index;
        for &block in memberships {
            common_articulation_checkpoint_v1(checkpoint)?;
            adjacency[block].push(articulation_node);
            adjacency[articulation_node].push(block);
        }
    }

    let mut visited = vec![false; node_count];
    let mut queue = VecDeque::from([0usize]);
    visited[0] = true;
    while let Some(node) = queue.pop_front() {
        common_articulation_checkpoint_v1(checkpoint)?;
        for &next in &adjacency[node] {
            common_articulation_checkpoint_v1(checkpoint)?;
            if !visited[next] {
                visited[next] = true;
                queue.push_back(next);
            }
        }
    }
    for seen in visited {
        common_articulation_checkpoint_v1(checkpoint)?;
        if !seen {
            return Ok(false);
        }
    }
    Ok(true)
}

fn angle_bits_v1(angles: &CanonicalHingeAngles, edge: EdgeId) -> Option<u64> {
    angles
        .as_slice()
        .binary_search_by_key(&edge.canonical_bytes(), |angle| {
            angle.edge().canonical_bytes()
        })
        .ok()
        .map(|index| angles.as_slice()[index].angle_degrees().to_bits())
}

fn retained_bytes_v1(
    block_count: usize,
    submitted_faces: usize,
    submitted_hinges: usize,
    articulation_faces: usize,
) -> Result<usize, CommonArticulationPoseErrorV1> {
    AUTHORITY_BASE_BYTES_V1
        .checked_add(
            block_count
                .checked_mul(BLOCK_RECORD_BYTES_V1)
                .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)?,
        )
        .and_then(|value| {
            value.checked_add(submitted_faces.checked_mul(FACE_TRANSFORM_RECORD_BYTES_V1)?)
        })
        .and_then(|value| {
            value.checked_add(submitted_hinges.checked_mul(HINGE_ANGLE_RECORD_BYTES_V1)?)
        })
        .and_then(|value| {
            value.checked_add(articulation_faces.checked_mul(ARTICULATION_FACE_RECORD_BYTES_V1)?)
        })
        .ok_or(CommonArticulationPoseErrorV1::ResourceLimit)
}

fn rigid_transform_is_finite_v1(transform: RigidTransform) -> bool {
    transform
        .rotation_rows()
        .into_iter()
        .flatten()
        .chain([
            transform.translation().x(),
            transform.translation().y(),
            transform.translation().z(),
        ])
        .all(f64::is_finite)
}

fn rigid_transform_bits_v1(transform: RigidTransform) -> [u64; 12] {
    let rows = transform.rotation_rows();
    [
        rows[0][0].to_bits(),
        rows[0][1].to_bits(),
        rows[0][2].to_bits(),
        rows[1][0].to_bits(),
        rows[1][1].to_bits(),
        rows[1][2].to_bits(),
        rows[2][0].to_bits(),
        rows[2][1].to_bits(),
        rows[2][2].to_bits(),
        transform.translation().x().to_bits(),
        transform.translation().y().to_bits(),
        transform.translation().z().to_bits(),
    ]
}

fn candidate_slice_equal_with_checkpoint_v1<T: PartialEq>(
    first: &[T],
    second: &[T],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<bool, CommonArticulationPoseErrorV1> {
    if first.len() != second.len() {
        return Ok(false);
    }
    for (first, second) in first.iter().zip(second) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if first != second {
            return Ok(false);
        }
    }
    Ok(true)
}

fn candidate_face_transforms_bit_equal_with_checkpoint_v1(
    first: &[CandidateFaceTransform],
    second: &[CandidateFaceTransform],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<bool, CommonArticulationPoseErrorV1> {
    if first.len() != second.len() {
        return Ok(false);
    }
    for (first, second) in first.iter().zip(second) {
        common_articulation_checkpoint_v1(checkpoint)?;
        if first.face() != second.face()
            || rigid_transform_bits_v1(first.transform())
                != rigid_transform_bits_v1(second.transform())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn binding_fingerprint_with_checkpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    blocks: &[CommonArticulationPoseBlockRestrictionV1],
    articulation_faces: &[FaceId],
    paper_thickness_bits: u64,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<[u8; 32], CommonArticulationPoseErrorV1> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_POSE_MODEL_ID_V1.as_bytes());
    hash.update(paper_thickness_bits.to_le_bytes());
    hash.update(pose.fixed_face().canonical_bytes());
    for face in geometry.face_ids() {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(face.canonical_bytes());
        if let Some(transform) = pose.face_transform(*face) {
            for bits in rigid_transform_bits_v1(transform) {
                common_articulation_checkpoint_v1(checkpoint)?;
                hash.update(bits.to_le_bytes());
            }
        }
    }
    for angle in pose.hinge_angles().as_slice() {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_le_bytes());
    }
    for block in blocks {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update((block.faces.len() as u64).to_le_bytes());
        hash.update((block.hinge_angles.len() as u64).to_le_bytes());
        for face in &block.faces {
            common_articulation_checkpoint_v1(checkpoint)?;
            hash.update(face.canonical_bytes());
        }
        for hinge in &block.hinge_angles {
            common_articulation_checkpoint_v1(checkpoint)?;
            hash.update(hinge.edge.canonical_bytes());
            hash.update(hinge.angle_degrees_bits.to_le_bytes());
        }
    }
    for face in articulation_faces {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn extension_binding_fingerprint_with_checkpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    blocks: &[CommonArticulationPoseBlockRestrictionV1],
    articulation_faces: &[FaceId],
    paper_thickness_bits: u64,
    configured_max_blocks: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<[u8; 32], CommonArticulationPoseErrorV1> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_POSE_EXTENSION_MODEL_ID_V1.as_bytes());
    for value in [
        COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1 as u64,
        configured_max_blocks as u64,
        blocks.len() as u64,
    ] {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(value.to_le_bytes());
    }
    hash.update(paper_thickness_bits.to_le_bytes());
    hash.update(pose.fixed_face().canonical_bytes());
    for face in geometry.face_ids() {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(face.canonical_bytes());
        if let Some(transform) = pose.face_transform(*face) {
            for bits in rigid_transform_bits_v1(transform) {
                common_articulation_checkpoint_v1(checkpoint)?;
                hash.update(bits.to_le_bytes());
            }
        }
    }
    for angle in pose.hinge_angles().as_slice() {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_le_bytes());
    }
    for block in blocks {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update((block.faces.len() as u64).to_le_bytes());
        hash.update((block.hinge_angles.len() as u64).to_le_bytes());
        for face in &block.faces {
            common_articulation_checkpoint_v1(checkpoint)?;
            hash.update(face.canonical_bytes());
        }
        for hinge in &block.hinge_angles {
            common_articulation_checkpoint_v1(checkpoint)?;
            hash.update(hinge.edge.canonical_bytes());
            hash.update(hinge.angle_degrees_bits.to_le_bytes());
        }
    }
    for face in articulation_faces {
        common_articulation_checkpoint_v1(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn common_articulation_checkpoint_v1(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<(), CommonArticulationPoseErrorV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationPoseStopV1::Cancelled => CommonArticulationPoseErrorV1::Cancelled,
        CommonArticulationPoseStopV1::DeadlineExceeded => {
            CommonArticulationPoseErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalEdgeBlockLimitsV1, HingeAngle, Point3, TreeHinge};
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::FoldAssignment;

    struct ChainFixtureV1 {
        geometry: MaterialHingeGraphGeometry,
        pose: ClosedMaterialHingeGraphPose,
        decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    }

    impl ChainFixtureV1 {
        fn input(
            &self,
            paper_thickness_mm: f64,
            limits: CommonArticulationPoseLimitsV1,
        ) -> CommonArticulationPoseInputV1<'_> {
            CommonArticulationPoseInputV1 {
                geometry: &self.geometry,
                pose: &self.pose,
                decomposition: &self.decomposition,
                paper_thickness_mm,
                limits,
            }
        }

        fn extension_input(
            &self,
            paper_thickness_mm: f64,
            limits: CommonArticulationPoseExtensionLimitsV1,
        ) -> CommonArticulationPoseExtensionInputV1<'_> {
            CommonArticulationPoseExtensionInputV1 {
                geometry: &self.geometry,
                pose: &self.pose,
                decomposition: &self.decomposition,
                paper_thickness_mm,
                limits,
            }
        }
    }

    fn extension_limits_v1(max_blocks: usize) -> CommonArticulationPoseExtensionLimitsV1 {
        CommonArticulationPoseExtensionLimitsV1::with_max_blocks_v1(max_blocks)
            .expect("valid extension cap")
    }

    fn chain_fixture_v1(block_count: usize) -> ChainFixtureV1 {
        chain_fixture_with_namespace_v1(block_count, ProjectId::new())
    }

    fn chain_fixture_with_namespace_v1(block_count: usize, namespace: ProjectId) -> ChainFixtureV1 {
        let mut faces = (0..=block_count)
            .map(|index| {
                FaceId::derive_v5(
                    namespace,
                    &[
                        b"common-pose-face-v1".as_slice(),
                        &(index as u64).to_le_bytes(),
                    ]
                    .concat(),
                )
            })
            .collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let mut edges = (0..block_count)
            .map(|index| {
                EdgeId::derive_v5(
                    namespace,
                    &[
                        b"common-pose-edge-v1".as_slice(),
                        &(index as u64).to_le_bytes(),
                    ]
                    .concat(),
                )
            })
            .collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let start = Point3::new(0.0, 0.0, 0.0).expect("finite start");
        let end = Point3::new(1.0, 0.0, 0.0).expect("finite end");
        let axis = Point3::new(1.0, 0.0, 0.0).expect("unit axis");
        let hinges = edges
            .iter()
            .copied()
            .enumerate()
            .map(|(index, edge)| {
                TreeHinge::new_for_test(
                    edge,
                    FoldAssignment::Mountain,
                    faces[index],
                    faces[index + 1],
                    start,
                    end,
                    axis,
                )
            })
            .collect::<Vec<_>>();
        let geometry = MaterialHingeGraphGeometry::new_for_test(faces.clone(), hinges.clone());
        let audit = super::super::MaterialHingeGraphAudit::from_block(&faces, &hinges)
            .expect("connected chain audit");
        let angles = CanonicalHingeAngles::new(
            edges
                .iter()
                .copied()
                .map(|edge| HingeAngle::new(edge, 0.0).expect("zero angle"))
                .collect(),
        )
        .expect("canonical angles");
        let pose = geometry
            .solve_closed(&audit, faces[0], &angles, 0.0)
            .expect("closed chain pose");
        let decomposition = geometry
            .decompose_canonical_edge_blocks_v1(
                &audit,
                CanonicalEdgeBlockLimitsV1 {
                    max_blocks: block_count,
                    max_faces_per_block: 2,
                    max_hinges_per_block: 2,
                },
            )
            .expect("canonical bridge decomposition");
        ChainFixtureV1 {
            geometry,
            pose,
            decomposition,
        }
    }

    fn shared_face_star_fixture_v1(block_count: usize) -> ChainFixtureV1 {
        let articulation = FaceId::new();
        let exclusive_faces = (0..block_count)
            .map(|_| std::array::from_fn::<_, 5, _>(|_| FaceId::new()))
            .collect::<Vec<_>>();
        let mut faces = vec![articulation];
        faces.extend(exclusive_faces.iter().flatten().copied());
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let start = Point3::new(0.0, 0.0, 0.0).expect("finite start");
        let end = Point3::new(1.0, 0.0, 0.0).expect("finite end");
        let axis = Point3::new(1.0, 0.0, 0.0).expect("unit axis");
        let mut hinges = Vec::new();
        for exclusive in &exclusive_faces {
            let cycle = [
                articulation,
                exclusive[0],
                exclusive[1],
                exclusive[2],
                exclusive[3],
                exclusive[4],
            ];
            for index in 0..cycle.len() {
                hinges.push(TreeHinge::new_for_test(
                    EdgeId::new(),
                    FoldAssignment::Mountain,
                    cycle[index],
                    cycle[(index + 1) % cycle.len()],
                    start,
                    end,
                    axis,
                ));
            }
        }
        let geometry = MaterialHingeGraphGeometry::new_for_test(faces.clone(), hinges.clone());
        let audit = super::super::MaterialHingeGraphAudit::from_block(&faces, &hinges)
            .expect("connected shared-face star audit");
        let mut angles = geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero angle"))
            .collect::<Vec<_>>();
        angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
        let angles = CanonicalHingeAngles::new(angles).expect("canonical angles");
        let pose = geometry
            .solve_closed(&audit, articulation, &angles, 0.0)
            .expect("closed shared-face star pose");
        let decomposition = geometry
            .decompose_canonical_edge_blocks_v1(
                &audit,
                CanonicalEdgeBlockLimitsV1 {
                    max_blocks: block_count,
                    max_faces_per_block: 6,
                    max_hinges_per_block: 6,
                },
            )
            .expect("canonical shared-face star decomposition");
        ChainFixtureV1 {
            geometry,
            pose,
            decomposition,
        }
    }

    fn observed_transform_bits_v1(transform: RigidTransform) -> [u64; 12] {
        let rotation = transform.rotation_rows();
        let translation = transform.translation();
        [
            rotation[0][0].to_bits(),
            rotation[0][1].to_bits(),
            rotation[0][2].to_bits(),
            rotation[1][0].to_bits(),
            rotation[1][1].to_bits(),
            rotation[1][2].to_bits(),
            rotation[2][0].to_bits(),
            rotation[2][1].to_bits(),
            rotation[2][2].to_bits(),
            translation.x().to_bits(),
            translation.y().to_bits(),
            translation.z().to_bits(),
        ]
    }

    fn direct_legacy_pose_binding_v1(
        authority: &CommonArticulationPoseAuthorityV1,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"common_articulation_pose_authority_v1");
        hash.update(authority.paper_thickness_mm_v1().to_bits().to_le_bytes());
        hash.update(pose.fixed_face().canonical_bytes());
        for face in geometry.face_ids() {
            hash.update(face.canonical_bytes());
            if let Some(transform) = pose.face_transform(*face) {
                for bits in observed_transform_bits_v1(transform) {
                    hash.update(bits.to_le_bytes());
                }
            }
        }
        for angle in pose.hinge_angles().as_slice() {
            hash.update(angle.edge().canonical_bytes());
            hash.update(angle.angle_degrees().to_bits().to_le_bytes());
        }
        for index in 0..authority.block_count_v1() {
            let block = authority.block_v1(index).expect("observed legacy block");
            hash.update((block.face_ids().len() as u64).to_le_bytes());
            hash.update((block.hinge_angles().len() as u64).to_le_bytes());
            for face in block.face_ids() {
                hash.update(face.canonical_bytes());
            }
            for hinge in block.hinge_angles() {
                hash.update(hinge.edge().canonical_bytes());
                hash.update(hinge.angle_degrees_bits().to_le_bytes());
            }
        }
        for face in authority.articulation_faces_v1() {
            hash.update(face.canonical_bytes());
        }
        hash.finalize().into()
    }

    fn direct_extension_pose_binding_v1(
        authority: &CommonArticulationPoseExtensionAuthorityV1,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
        configured_max_blocks: usize,
        actual_count: usize,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"common_articulation_pose_extension_authority_v1");
        for value in [11_u64, configured_max_blocks as u64, actual_count as u64] {
            hash.update(value.to_le_bytes());
        }
        hash.update(authority.paper_thickness_mm_v1().to_bits().to_le_bytes());
        hash.update(pose.fixed_face().canonical_bytes());
        for face in geometry.face_ids() {
            hash.update(face.canonical_bytes());
            if let Some(transform) = pose.face_transform(*face) {
                for bits in observed_transform_bits_v1(transform) {
                    hash.update(bits.to_le_bytes());
                }
            }
        }
        for angle in pose.hinge_angles().as_slice() {
            hash.update(angle.edge().canonical_bytes());
            hash.update(angle.angle_degrees().to_bits().to_le_bytes());
        }
        for index in 0..authority.block_count_v1() {
            let block = authority.block_v1(index).expect("observed extension block");
            hash.update((block.face_ids().len() as u64).to_le_bytes());
            hash.update((block.hinge_angles().len() as u64).to_le_bytes());
            for face in block.face_ids() {
                hash.update(face.canonical_bytes());
            }
            for hinge in block.hinge_angles() {
                hash.update(hinge.edge().canonical_bytes());
                hash.update(hinge.angle_degrees_bits().to_le_bytes());
            }
        }
        for face in authority.articulation_faces_v1() {
            hash.update(face.canonical_bytes());
        }
        hash.finalize().into()
    }

    fn digest_hex_v1(digest: [u8; 32]) -> String {
        use std::fmt::Write as _;

        digest
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                write!(hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }

    #[test]
    fn legacy_two_through_ten_binding_and_revalidation_bytes_remain_frozen() {
        assert_eq!(COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1, 10);
        assert_eq!(CommonArticulationPoseLimitsV1::default().max_blocks, 8);
        let expected = [
            "89916ff935c87466a9135cebac43e2eea483f5d8baea2261ce7308a2b1b7844c",
            "073caf367bf96cae617eb4a2661ddaac6ca1890801b149143497d83999269f33",
            "04b2a07d08dc7b5f8f7071b8d20c14a22b132ff8434c164050afd324b5cd1965",
            "19846611374feca215d541d11bdb9c73606d5fd7deebc967c1399c79fcd1669d",
            "30adc01ef88fa5bee017aba65388d9fd6b83c175147eb4b7b95408adc3afb00c",
            "3a7183ba8b6c106ebd4269d07e98ab2ddf7d5873e27f85f4e3d147c627b3accc",
            "1d27de53b30e121767e4d3ba3fb13b094ffb2cdeb7225c362cfe7fd1f3e15d7f",
            "f5108a20bc26481436c20c23159172d2e18e5048484622b96e9765331ef5fc4b",
            "07656efb0090e16f3026cbea351281fe7189a849258c5ff16534d31c1383e913",
        ];

        for (block_count, expected_digest) in (2..=10).zip(expected) {
            let fixture = chain_fixture_with_namespace_v1(
                block_count,
                ProjectId::schema_namespace([0x6c; 16]),
            );
            let limits = if block_count <= 8 {
                CommonArticulationPoseLimitsV1::default()
            } else {
                CommonArticulationPoseLimitsV1 {
                    max_blocks: 10,
                    ..CommonArticulationPoseLimitsV1::default()
                }
            };
            let authority = prove_common_articulation_pose_authority_v1(fixture.input(0.1, limits))
                .expect("legacy corpus authority");
            let repeated = prove_common_articulation_pose_authority_v1(fixture.input(0.1, limits))
                .expect("repeated legacy corpus authority");
            assert_eq!(
                authority.binding_fingerprint_v1(),
                repeated.binding_fingerprint_v1(),
                "legacy arity {block_count} must issue deterministically",
            );
            assert_eq!(
                digest_hex_v1(authority.binding_fingerprint_v1()),
                expected_digest,
                "legacy arity {block_count} golden digest changed",
            );
            assert_eq!(
                authority.binding_fingerprint_v1(),
                direct_legacy_pose_binding_v1(&authority, &fixture.geometry, &fixture.pose),
                "legacy arity {block_count} byte-update algorithm changed",
            );
            authority
                .revalidate_v1(fixture.input(0.1, limits))
                .expect("legacy corpus revalidation");
        }
    }

    fn assert_extension_cap_pair_v1(
        fixture: &ChainFixtureV1,
        actual_count: usize,
        current_cap: usize,
        replay_cap: usize,
    ) {
        assert_eq!(fixture.decomposition.blocks().len(), actual_count);
        let current_limits = extension_limits_v1(current_cap);
        let replay_limits = extension_limits_v1(replay_cap);
        let current = prove_common_articulation_pose_extension_authority_v1(
            fixture.extension_input(0.1, current_limits),
        )
        .expect("current-cap extension authority");
        let replay = prove_common_articulation_pose_extension_authority_v1(
            fixture.extension_input(0.1, replay_limits),
        )
        .expect("replay-cap extension authority");

        assert_eq!(
            current.model_id(),
            COMMON_ARTICULATION_POSE_EXTENSION_MODEL_ID_V1
        );
        assert_eq!(current.block_count_v1(), actual_count);
        assert_eq!(current.configured_max_blocks_v1(), current_cap);
        assert_eq!(replay.configured_max_blocks_v1(), replay_cap);
        assert_eq!(
            current.binding_fingerprint_v1(),
            direct_extension_pose_binding_v1(
                &current,
                &fixture.geometry,
                &fixture.pose,
                current_cap,
                actual_count,
            ),
        );
        assert_eq!(
            replay.binding_fingerprint_v1(),
            direct_extension_pose_binding_v1(
                &replay,
                &fixture.geometry,
                &fixture.pose,
                replay_cap,
                actual_count,
            ),
        );
        assert_ne!(
            current.binding_fingerprint_v1(),
            replay.binding_fingerprint_v1(),
            "a configured-cap replay must change the authority binding",
        );
        current
            .revalidate_v1(fixture.extension_input(0.1, current_limits))
            .expect("current-cap extension revalidation");
        replay
            .revalidate_v1(fixture.extension_input(0.1, replay_limits))
            .expect("replay-cap extension revalidation");
        assert_eq!(
            current
                .revalidate_v1(fixture.extension_input(0.1, replay_limits))
                .expect_err("foreign configured cap"),
            CommonArticulationPoseErrorV1::IssuerMismatch,
        );
        assert_eq!(
            replay
                .revalidate_v1(fixture.extension_input(0.1, current_limits))
                .expect_err("reverse configured-cap replay"),
            CommonArticulationPoseErrorV1::IssuerMismatch,
        );
        assert!(!current.authorizes_continuous_motion());
        assert!(!current.authorizes_collision_clearance());
        assert!(!current.authorizes_project_mutation());
        assert!(!current.authorizes_apply());
        assert!(!current.authorizes_viewer());
    }

    #[test]
    fn extension_eleven_and_twelve_bind_actual_and_configured_caps_in_order() {
        assert_eq!(COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1, 11);
        assert_eq!(COMMON_ARTICULATION_POSE_EXTENSION_MAX_BLOCKS_V1, 32);
        assert_extension_cap_pair_v1(&chain_fixture_v1(11), 11, 11, 12);
        // [min=11, cap=13, actual=12] makes all three fixed-order fields distinct.
        assert_extension_cap_pair_v1(&chain_fixture_v1(12), 12, 12, 13);
    }

    #[test]
    fn extension_foreign_pose_instance_fails_closed() {
        let fixture = chain_fixture_v1(11);
        let limits = extension_limits_v1(11);
        let authority = prove_common_articulation_pose_extension_authority_v1(
            fixture.extension_input(0.1, limits),
        )
        .expect("extension authority");
        let audit = super::super::MaterialHingeGraphAudit::from_block(
            fixture.geometry.face_ids(),
            fixture.geometry.hinges(),
        )
        .expect("same-geometry audit");
        let foreign_pose = fixture
            .geometry
            .solve_closed(
                &audit,
                fixture.pose.fixed_face(),
                fixture.pose.hinge_angles(),
                0.0,
            )
            .expect("separately issued equal pose");
        assert_eq!(
            authority
                .revalidate_v1(CommonArticulationPoseExtensionInputV1 {
                    pose: &foreign_pose,
                    ..fixture.extension_input(0.1, limits)
                })
                .expect_err("foreign pose instance"),
            CommonArticulationPoseErrorV1::IssuerMismatch,
        );
    }

    #[test]
    fn extension_exact_resource_envelope_and_invalid_or_overflow_limits_fail_closed() {
        let fixture = chain_fixture_v1(11);
        let baseline_limits = extension_limits_v1(11);
        let baseline = prove_common_articulation_pose_extension_authority_v1(
            fixture.extension_input(0.1, baseline_limits),
        )
        .expect("extension resource baseline");
        let exact = CommonArticulationPoseExtensionLimitsV1 {
            max_blocks: 11,
            max_faces: fixture.geometry.face_ids().len(),
            max_hinges: fixture.geometry.hinges().len(),
            max_work: baseline.logical_work_v1(),
            max_retained_bytes: baseline.retained_bytes_upper_bound_v1(),
        };
        prove_common_articulation_pose_extension_authority_v1(fixture.extension_input(0.1, exact))
            .expect("exact extension resource envelope");

        for one_short in [
            CommonArticulationPoseExtensionLimitsV1 {
                max_blocks: exact.max_blocks - 1,
                ..exact
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_faces: exact.max_faces - 1,
                ..exact
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_hinges: exact.max_hinges - 1,
                ..exact
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_work: exact.max_work - 1,
                ..exact
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_retained_bytes: exact.max_retained_bytes - 1,
                ..exact
            },
        ] {
            assert_eq!(
                prove_common_articulation_pose_extension_authority_v1(
                    fixture.extension_input(0.1, one_short),
                )
                .expect_err("one-short extension resource"),
                CommonArticulationPoseErrorV1::ResourceLimit,
            );
        }

        for invalid_cap in [0, 10, 33, usize::MAX] {
            assert!(
                CommonArticulationPoseExtensionLimitsV1::with_max_blocks_v1(invalid_cap).is_none()
            );
            assert_eq!(
                prove_common_articulation_pose_extension_authority_v1(fixture.extension_input(
                    0.1,
                    CommonArticulationPoseExtensionLimitsV1 {
                        max_blocks: invalid_cap,
                        ..baseline_limits
                    },
                ),)
                .expect_err("invalid extension configured cap"),
                CommonArticulationPoseErrorV1::ResourceLimit,
            );
        }
        for overflow in [
            CommonArticulationPoseExtensionLimitsV1 {
                max_faces: usize::MAX,
                ..baseline_limits
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_hinges: usize::MAX,
                ..baseline_limits
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_work: usize::MAX,
                ..baseline_limits
            },
            CommonArticulationPoseExtensionLimitsV1 {
                max_retained_bytes: usize::MAX,
                ..baseline_limits
            },
        ] {
            assert_eq!(
                prove_common_articulation_pose_extension_authority_v1(
                    fixture.extension_input(0.1, overflow),
                )
                .expect_err("overflowing extension resource limit"),
                CommonArticulationPoseErrorV1::ResourceLimit,
            );
        }
        assert_eq!(
            retained_bytes_v1(usize::MAX, usize::MAX, usize::MAX, usize::MAX)
                .expect_err("checked retained-byte overflow"),
            CommonArticulationPoseErrorV1::ResourceLimit,
        );
    }

    #[test]
    fn extension_hard_thirty_two_boundary_is_inclusive_and_fails_closed_above() {
        let eleven = chain_fixture_v1(11);
        let cap_thirty_two = extension_limits_v1(32);
        let eleven_under_hard_cap = prove_common_articulation_pose_extension_authority_v1(
            eleven.extension_input(0.1, cap_thirty_two),
        )
        .expect("eleven actual blocks under hard cap");
        assert_eq!(eleven_under_hard_cap.configured_max_blocks_v1(), 32);

        let thirty_two = chain_fixture_v1(32);
        let authority = prove_common_articulation_pose_extension_authority_v1(
            thirty_two.extension_input(0.1, cap_thirty_two),
        )
        .expect("inclusive thirty-two-block boundary");
        assert_eq!(authority.block_count_v1(), 32);
        assert_eq!(
            authority.binding_fingerprint_v1(),
            direct_extension_pose_binding_v1(
                &authority,
                &thirty_two.geometry,
                &thirty_two.pose,
                32,
                32,
            ),
        );
        authority
            .revalidate_v1(thirty_two.extension_input(0.1, cap_thirty_two))
            .expect("thirty-two-block revalidation");

        let ten = chain_fixture_v1(10);
        assert_eq!(
            prove_common_articulation_pose_extension_authority_v1(
                ten.extension_input(0.1, extension_limits_v1(11)),
            )
            .expect_err("actual count below extension minimum"),
            CommonArticulationPoseErrorV1::ResourceLimit,
        );
    }

    #[test]
    fn extension_issuance_and_revalidation_honor_cancel_and_deadline() {
        let fixture = chain_fixture_v1(11);
        let input = fixture.extension_input(0.1, extension_limits_v1(11));
        let mut issuance_checkpoints = 0usize;
        prove_common_articulation_pose_extension_authority_with_checkpoint_v1(input, || {
            issuance_checkpoints += 1;
            Ok(())
        })
        .expect("count extension issuance checkpoints");
        assert!(issuance_checkpoints > 4);
        for stop_at in [1, issuance_checkpoints / 2, issuance_checkpoints] {
            for (stop, expected) in [
                (
                    CommonArticulationPoseStopV1::Cancelled,
                    CommonArticulationPoseErrorV1::Cancelled,
                ),
                (
                    CommonArticulationPoseStopV1::DeadlineExceeded,
                    CommonArticulationPoseErrorV1::DeadlineExceeded,
                ),
            ] {
                let mut observed = 0usize;
                assert_eq!(
                    prove_common_articulation_pose_extension_authority_with_checkpoint_v1(
                        input,
                        || {
                            observed += 1;
                            if observed == stop_at {
                                Err(stop)
                            } else {
                                Ok(())
                            }
                        },
                    )
                    .expect_err("extension issuance stop"),
                    expected,
                );
            }
        }

        let authority = prove_common_articulation_pose_extension_authority_v1(input)
            .expect("extension authority for revalidation stop");
        let mut revalidation_checkpoints = 0usize;
        authority
            .revalidate_with_checkpoint_v1(input, || {
                revalidation_checkpoints += 1;
                Ok(())
            })
            .expect("count extension revalidation checkpoints");
        assert!(revalidation_checkpoints > issuance_checkpoints + 4);
        for stop_at in [1, issuance_checkpoints + 2, revalidation_checkpoints] {
            for (stop, expected) in [
                (
                    CommonArticulationPoseStopV1::Cancelled,
                    CommonArticulationPoseErrorV1::Cancelled,
                ),
                (
                    CommonArticulationPoseStopV1::DeadlineExceeded,
                    CommonArticulationPoseErrorV1::DeadlineExceeded,
                ),
            ] {
                let mut observed = 0usize;
                assert_eq!(
                    authority
                        .revalidate_with_checkpoint_v1(input, || {
                            observed += 1;
                            if observed == stop_at {
                                Err(stop)
                            } else {
                                Ok(())
                            }
                        })
                        .expect_err("extension revalidation stop"),
                    expected,
                );
                assert_eq!(observed, stop_at);
            }
        }
    }

    #[test]
    fn two_three_five_eight_nine_and_ten_block_chains_issue_exact_parent_pose_restrictions() {
        for block_count in [2, 3, 5, 8, 9, 10] {
            let fixture = chain_fixture_v1(block_count);
            let limits = if block_count > COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1 {
                CommonArticulationPoseLimitsV1 {
                    max_blocks: block_count,
                    ..CommonArticulationPoseLimitsV1::default()
                }
            } else {
                CommonArticulationPoseLimitsV1::default()
            };
            let authority = prove_common_articulation_pose_authority_v1(fixture.input(0.1, limits))
                .expect("bounded common pose authority");
            assert_eq!(authority.block_count_v1(), block_count);
            assert_eq!(authority.limits.max_blocks, limits.max_blocks);
            assert_eq!(authority.articulation_faces_v1().len(), block_count - 1);
            assert_eq!(
                authority.paper_thickness_mm_v1().to_bits(),
                0.1_f64.to_bits()
            );
            assert!(!authority.authorizes_continuous_motion());
            assert!(!authority.authorizes_collision_clearance());
            assert!(!authority.authorizes_project_mutation());
            assert!(!authority.authorizes_apply());
            assert!(!authority.authorizes_viewer());
            for index in 0..block_count {
                let block = authority.block_v1(index).expect("retained block");
                let block_geometry = fixture.decomposition.blocks()[index].geometry();
                assert!(block.is_for_geometry(block_geometry));
                assert_eq!(
                    block.face_transforms().len(),
                    block_geometry.face_ids().len()
                );
                assert_eq!(block.face_ids(), block_geometry.face_ids());
                assert_eq!(block.hinge_angles().len(), block_geometry.hinges().len());
                for transform in block.face_transforms() {
                    assert_eq!(
                        rigid_transform_bits_v1(transform.transform()),
                        rigid_transform_bits_v1(
                            fixture
                                .pose
                                .face_transform(transform.face())
                                .expect("parent transform"),
                        ),
                    );
                }
            }
            authority
                .revalidate_v1(fixture.input(0.1, limits))
                .expect("exact live revalidation");
        }
    }

    #[test]
    fn default_block_cap_preserves_eight_block_compatibility_while_hard_cap_is_ten() {
        assert_eq!(
            CommonArticulationPoseLimitsV1::default().max_blocks,
            COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1
        );
        assert_eq!(COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1, 8);
        assert_eq!(COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1, 10);

        let nine_block_limits = CommonArticulationPoseLimitsV1 {
            max_blocks: 9,
            ..CommonArticulationPoseLimitsV1::default()
        };
        let fixture = chain_fixture_v1(9);
        let authority =
            prove_common_articulation_pose_authority_v1(fixture.input(0.1, nine_block_limits))
                .expect("nine-block caller limit remains exact");
        assert_eq!(authority.limits.max_blocks, 9);
    }

    #[test]
    fn nine_block_pose_cap_one_short_fails_closed_before_authority_issuance() {
        let fixture = chain_fixture_v1(9);
        assert_eq!(
            prove_common_articulation_pose_authority_v1(fixture.input(
                0.1,
                CommonArticulationPoseLimitsV1 {
                    max_blocks: 8,
                    ..CommonArticulationPoseLimitsV1::default()
                },
            ))
            .expect_err("nine blocks exceed the one-short pose cap"),
            CommonArticulationPoseErrorV1::ResourceLimit
        );
    }

    #[test]
    fn four_blocks_sharing_one_articulation_face_form_one_incidence_tree() {
        let fixture = shared_face_star_fixture_v1(4);
        assert_eq!(fixture.decomposition.blocks().len(), 4);
        assert_eq!(fixture.decomposition.articulation_faces().len(), 1);
        assert!(fixture.decomposition.blocks().iter().all(|block| {
            block
                .geometry()
                .face_ids()
                .contains(&fixture.pose.fixed_face())
        }));
        let authority = prove_common_articulation_pose_authority_v1(
            fixture.input(0.1, CommonArticulationPoseLimitsV1::default()),
        )
        .expect("shared-face star is a block-articulation incidence tree");
        assert_eq!(authority.block_count_v1(), 4);
        assert_eq!(
            authority.articulation_faces_v1(),
            fixture.decomposition.articulation_faces()
        );
    }

    #[test]
    fn exact_resource_envelope_passes_and_each_one_short_limit_fails_closed() {
        let fixture = chain_fixture_v1(3);
        let baseline = prove_common_articulation_pose_authority_v1(
            fixture.input(0.1, CommonArticulationPoseLimitsV1::default()),
        )
        .expect("baseline");
        let exact = CommonArticulationPoseLimitsV1 {
            max_blocks: 3,
            max_faces: fixture.geometry.face_ids().len(),
            max_hinges: fixture.geometry.hinges().len(),
            max_work: baseline.logical_work_v1(),
            max_retained_bytes: baseline.retained_bytes_upper_bound_v1(),
        };
        prove_common_articulation_pose_authority_v1(fixture.input(0.1, exact))
            .expect("exact resource limits");

        for one_short in [
            CommonArticulationPoseLimitsV1 {
                max_blocks: exact.max_blocks - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_faces: exact.max_faces - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_hinges: exact.max_hinges - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_work: exact.max_work - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_retained_bytes: exact.max_retained_bytes - 1,
                ..exact
            },
        ] {
            assert_eq!(
                prove_common_articulation_pose_authority_v1(fixture.input(0.1, one_short))
                    .expect_err("one-short limit"),
                CommonArticulationPoseErrorV1::ResourceLimit
            );
        }
    }

    #[test]
    fn exact_ten_block_resource_envelope_passes_and_each_one_short_limit_fails_closed() {
        let fixture = shared_face_star_fixture_v1(10);
        let baseline_limits = CommonArticulationPoseLimitsV1 {
            max_blocks: 10,
            ..CommonArticulationPoseLimitsV1::default()
        };
        let baseline =
            prove_common_articulation_pose_authority_v1(fixture.input(0.1, baseline_limits))
                .expect("ten-block baseline");
        assert_eq!(fixture.geometry.face_ids().len(), 51);
        assert_eq!(fixture.geometry.hinges().len(), 60);
        assert_eq!(baseline.logical_work_v1(), 2_864);
        assert_eq!(baseline.retained_bytes_upper_bound_v1(), 12_048);

        let exact = CommonArticulationPoseLimitsV1 {
            max_blocks: 10,
            max_faces: 51,
            max_hinges: 60,
            max_work: 2_864,
            max_retained_bytes: 12_048,
        };
        prove_common_articulation_pose_authority_v1(fixture.input(0.1, exact))
            .expect("exact ten-block resource limits");

        for one_short in [
            CommonArticulationPoseLimitsV1 {
                max_blocks: exact.max_blocks - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_faces: exact.max_faces - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_hinges: exact.max_hinges - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_work: exact.max_work - 1,
                ..exact
            },
            CommonArticulationPoseLimitsV1 {
                max_retained_bytes: exact.max_retained_bytes - 1,
                ..exact
            },
        ] {
            assert_eq!(
                prove_common_articulation_pose_authority_v1(fixture.input(0.1, one_short))
                    .expect_err("one-short ten-block limit"),
                CommonArticulationPoseErrorV1::ResourceLimit
            );
        }
    }

    #[test]
    fn eleven_blocks_and_nonpositive_or_nonfinite_thickness_are_rejected() {
        let fixture = chain_fixture_v1(11);
        assert_eq!(
            prove_common_articulation_pose_authority_v1(fixture.input(
                0.1,
                CommonArticulationPoseLimitsV1 {
                    max_blocks: COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1,
                    ..CommonArticulationPoseLimitsV1::default()
                },
            ))
            .expect_err("eleven blocks exceed the hard cap"),
            CommonArticulationPoseErrorV1::ResourceLimit
        );
        let fixture = chain_fixture_v1(2);
        for thickness in [0.0, -0.0, -0.1, f64::NAN, f64::INFINITY] {
            assert_eq!(
                prove_common_articulation_pose_authority_v1(
                    fixture.input(thickness, CommonArticulationPoseLimitsV1::default(),)
                )
                .expect_err("invalid thickness"),
                CommonArticulationPoseErrorV1::InvalidInput
            );
        }
    }

    #[test]
    fn pose_geometry_decomposition_angle_fixed_face_and_thickness_drift_fail_closed() {
        let fixture = chain_fixture_v1(3);
        let authority = prove_common_articulation_pose_authority_v1(
            fixture.input(0.1, CommonArticulationPoseLimitsV1::default()),
        )
        .expect("authority");

        let audit = super::super::MaterialHingeGraphAudit::from_block(
            fixture.geometry.face_ids(),
            fixture.geometry.hinges(),
        )
        .expect("audit");
        let separate_pose = fixture
            .geometry
            .solve_closed(
                &audit,
                fixture.pose.fixed_face(),
                fixture.pose.hinge_angles(),
                0.0,
            )
            .expect("separately issued pose");
        assert_eq!(
            authority
                .revalidate_v1(CommonArticulationPoseInputV1 {
                    pose: &separate_pose,
                    ..fixture.input(0.1, CommonArticulationPoseLimitsV1::default())
                })
                .expect_err("ABA pose"),
            CommonArticulationPoseErrorV1::IssuerMismatch
        );

        let mut one_ulp_angles = fixture.pose.hinge_angles().as_slice().to_vec();
        one_ulp_angles[0] = HingeAngle::new(
            one_ulp_angles[0].edge(),
            f64::from_bits(one_ulp_angles[0].angle_degrees().to_bits() + 1),
        )
        .expect("one ulp");
        let one_ulp_angles =
            CanonicalHingeAngles::new(one_ulp_angles).expect("canonical one-ulp angles");
        let one_ulp_pose = fixture
            .geometry
            .solve_closed(&audit, fixture.pose.fixed_face(), &one_ulp_angles, 1.0e-9)
            .expect("one-ulp pose");
        assert_eq!(
            authority
                .revalidate_v1(CommonArticulationPoseInputV1 {
                    pose: &one_ulp_pose,
                    ..fixture.input(0.1, CommonArticulationPoseLimitsV1::default())
                })
                .expect_err("angle drift"),
            CommonArticulationPoseErrorV1::IssuerMismatch
        );

        let alternate_fixed = fixture
            .geometry
            .face_ids()
            .iter()
            .copied()
            .find(|face| *face != fixture.pose.fixed_face())
            .expect("alternate fixed face");
        let alternate_pose = fixture
            .geometry
            .solve_closed(&audit, alternate_fixed, fixture.pose.hinge_angles(), 0.0)
            .expect("alternate fixed pose");
        assert_eq!(
            authority
                .revalidate_v1(CommonArticulationPoseInputV1 {
                    pose: &alternate_pose,
                    ..fixture.input(0.1, CommonArticulationPoseLimitsV1::default())
                })
                .expect_err("fixed-face drift"),
            CommonArticulationPoseErrorV1::IssuerMismatch
        );

        assert_eq!(
            authority
                .revalidate_v1(fixture.input(
                    f64::from_bits(0.1_f64.to_bits() + 1),
                    CommonArticulationPoseLimitsV1::default(),
                ))
                .expect_err("thickness drift"),
            CommonArticulationPoseErrorV1::IssuerMismatch
        );

        let foreign_geometry = MaterialHingeGraphGeometry::new_for_test(
            fixture.geometry.face_ids().to_vec(),
            fixture.geometry.hinges().to_vec(),
        );
        let foreign_audit = super::super::MaterialHingeGraphAudit::from_block(
            foreign_geometry.face_ids(),
            foreign_geometry.hinges(),
        )
        .expect("foreign audit");
        let foreign_pose = foreign_geometry
            .solve_closed(
                &foreign_audit,
                fixture.pose.fixed_face(),
                fixture.pose.hinge_angles(),
                0.0,
            )
            .expect("foreign pose");
        let foreign_decomposition = foreign_geometry
            .decompose_canonical_edge_blocks_v1(&foreign_audit, fixture.decomposition.limits())
            .expect("foreign decomposition");
        assert_eq!(
            authority
                .revalidate_v1(CommonArticulationPoseInputV1 {
                    geometry: &foreign_geometry,
                    pose: &foreign_pose,
                    decomposition: &foreign_decomposition,
                    paper_thickness_mm: 0.1,
                    limits: CommonArticulationPoseLimitsV1::default(),
                })
                .expect_err("foreign equal geometry"),
            CommonArticulationPoseErrorV1::IssuerMismatch
        );
    }

    #[test]
    fn missing_extra_duplicate_and_foreign_block_sets_fail_before_mint() {
        let fixture = chain_fixture_v1(3);
        let input = fixture.input(0.1, CommonArticulationPoseLimitsV1::default());

        let mut missing = fixture.decomposition.clone();
        missing.blocks.pop();
        assert_eq!(
            prove_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
                decomposition: &missing,
                ..input
            })
            .expect_err("missing block"),
            CommonArticulationPoseErrorV1::InvalidDecomposition
        );

        let mut extra = fixture.decomposition.clone();
        let duplicate_block = extra.blocks[0].clone();
        extra.blocks.push(duplicate_block);
        assert_eq!(
            prove_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
                decomposition: &extra,
                ..input
            })
            .expect_err("extra block"),
            CommonArticulationPoseErrorV1::InvalidDecomposition
        );

        let mut duplicate = fixture.decomposition.clone();
        let duplicate_block = duplicate.blocks[0].clone();
        duplicate.blocks[1] = duplicate_block;
        assert_eq!(
            prove_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
                decomposition: &duplicate,
                ..input
            })
            .expect_err("duplicate overlapping block"),
            CommonArticulationPoseErrorV1::InvalidDecomposition
        );

        let foreign = chain_fixture_v1(3);
        assert_eq!(
            prove_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
                decomposition: &foreign.decomposition,
                ..input
            })
            .expect_err("foreign decomposition"),
            CommonArticulationPoseErrorV1::DecompositionIssuerMismatch
        );
    }

    #[test]
    fn cancellation_and_deadline_at_entry_midpoint_and_prepublication_return_no_authority() {
        let fixture = chain_fixture_v1(3);
        let input = fixture.input(0.1, CommonArticulationPoseLimitsV1::default());
        let mut checkpoints = 0usize;
        prove_common_articulation_pose_authority_with_checkpoint_v1(input, || {
            checkpoints += 1;
            Ok(())
        })
        .expect("count checkpoints");
        assert!(checkpoints > 4);

        for stop_at in [1, checkpoints / 2, checkpoints] {
            for (stop, expected) in [
                (
                    CommonArticulationPoseStopV1::Cancelled,
                    CommonArticulationPoseErrorV1::Cancelled,
                ),
                (
                    CommonArticulationPoseStopV1::DeadlineExceeded,
                    CommonArticulationPoseErrorV1::DeadlineExceeded,
                ),
            ] {
                let mut observed = 0usize;
                assert_eq!(
                    prove_common_articulation_pose_authority_with_checkpoint_v1(input, || {
                        observed += 1;
                        if observed == stop_at {
                            Err(stop)
                        } else {
                            Ok(())
                        }
                    })
                    .expect_err("typed cooperative stop"),
                    expected
                );
            }
        }
    }

    #[test]
    fn revalidation_stops_during_reproof_and_candidate_comparison() {
        let fixture = chain_fixture_v1(8);
        let input = fixture.input(0.1, CommonArticulationPoseLimitsV1::default());
        let authority =
            prove_common_articulation_pose_authority_v1(input).expect("baseline authority");

        let mut issuance_checkpoints = 0usize;
        prove_common_articulation_pose_authority_with_checkpoint_v1(input, || {
            issuance_checkpoints += 1;
            Ok(())
        })
        .expect("count issuance checkpoints");
        let mut revalidation_checkpoints = 0usize;
        authority
            .revalidate_with_checkpoint_v1(input, || {
                revalidation_checkpoints += 1;
                Ok(())
            })
            .expect("count revalidation checkpoints");
        assert!(revalidation_checkpoints > issuance_checkpoints + 4);

        for stop_at in [1, issuance_checkpoints + 2, revalidation_checkpoints] {
            for (stop, expected) in [
                (
                    CommonArticulationPoseStopV1::Cancelled,
                    CommonArticulationPoseErrorV1::Cancelled,
                ),
                (
                    CommonArticulationPoseStopV1::DeadlineExceeded,
                    CommonArticulationPoseErrorV1::DeadlineExceeded,
                ),
            ] {
                let mut observed = 0usize;
                assert_eq!(
                    authority
                        .revalidate_with_checkpoint_v1(input, || {
                            observed += 1;
                            if observed == stop_at {
                                Err(stop)
                            } else {
                                Ok(())
                            }
                        })
                        .expect_err("revalidation cooperative stop"),
                    expected
                );
                assert_eq!(observed, stop_at);
            }
        }
    }
}
