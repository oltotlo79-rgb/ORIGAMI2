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
pub const COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1: usize = 9;

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

pub fn prove_common_articulation_pose_authority_v1(
    input: CommonArticulationPoseInputV1<'_>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    prove_common_articulation_pose_authority_with_checkpoint_v1(input, || Ok(()))
}

pub fn prove_common_articulation_pose_authority_with_checkpoint_v1(
    input: CommonArticulationPoseInputV1<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV1>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    common_articulation_checkpoint_v1(&mut checkpoint)?;
    validate_limits_v1(input.limits)?;
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
    if !(COMMON_ARTICULATION_POSE_MIN_BLOCKS_V1..=COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1)
        .contains(&block_count)
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
        common_articulation_checkpoint_v1(&mut checkpoint)?;
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

    common_articulation_checkpoint_v1(&mut checkpoint)?;
    validate_parent_pose_v1(input.geometry, input.pose, &mut checkpoint)?;
    validate_decomposition_v1(input.geometry, input.decomposition, &mut checkpoint)?;

    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(block_count)
        .map_err(|_| CommonArticulationPoseErrorV1::ResourceLimit)?;
    for block in input.decomposition.blocks() {
        common_articulation_checkpoint_v1(&mut checkpoint)?;
        let mut face_transforms = Vec::new();
        face_transforms
            .try_reserve_exact(block.geometry().face_ids().len())
            .map_err(|_| CommonArticulationPoseErrorV1::ResourceLimit)?;
        for face in block.geometry().face_ids().iter().copied() {
            common_articulation_checkpoint_v1(&mut checkpoint)?;
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
            common_articulation_checkpoint_v1(&mut checkpoint)?;
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
            common_articulation_checkpoint_v1(&mut checkpoint)?;
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

    common_articulation_checkpoint_v1(&mut checkpoint)?;
    let binding_fingerprint = binding_fingerprint_with_checkpoint_v1(
        input.geometry,
        input.pose,
        &blocks,
        input.decomposition.articulation_faces(),
        input.paper_thickness_mm.to_bits(),
        &mut checkpoint,
    )?;
    common_articulation_checkpoint_v1(&mut checkpoint)?;
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
    use ori_domain::{EdgeId, FaceId};
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
    }

    fn chain_fixture_v1(block_count: usize) -> ChainFixtureV1 {
        let mut faces = (0..=block_count).map(|_| FaceId::new()).collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let mut edges = (0..block_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
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

    #[test]
    fn two_three_five_eight_and_nine_block_chains_issue_exact_parent_pose_restrictions() {
        for block_count in [2, 3, 5, 8, 9] {
            let fixture = chain_fixture_v1(block_count);
            let limits = if block_count == COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1 {
                CommonArticulationPoseLimitsV1 {
                    max_blocks: COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1,
                    ..CommonArticulationPoseLimitsV1::default()
                }
            } else {
                CommonArticulationPoseLimitsV1::default()
            };
            let authority = prove_common_articulation_pose_authority_v1(fixture.input(0.1, limits))
                .expect("bounded common pose authority");
            assert_eq!(authority.block_count_v1(), block_count);
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
    fn default_block_cap_preserves_eight_block_compatibility_while_hard_cap_is_nine() {
        assert_eq!(
            CommonArticulationPoseLimitsV1::default().max_blocks,
            COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1
        );
        assert_eq!(COMMON_ARTICULATION_POSE_DEFAULT_MAX_BLOCKS_V1, 8);
        assert_eq!(COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1, 9);
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
    fn ten_blocks_and_nonpositive_or_nonfinite_thickness_are_rejected() {
        let fixture = chain_fixture_v1(10);
        assert_eq!(
            prove_common_articulation_pose_authority_v1(fixture.input(
                0.1,
                CommonArticulationPoseLimitsV1 {
                    max_blocks: COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1,
                    ..CommonArticulationPoseLimitsV1::default()
                },
            ))
            .expect_err("ten blocks exceed the hard cap"),
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
