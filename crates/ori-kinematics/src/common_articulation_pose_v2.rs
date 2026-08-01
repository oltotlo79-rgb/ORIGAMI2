//! General-N, non-authorizing common-articulation pose provenance.
//!
//! This V2 path is intentionally separate from the 11..=32 V1 pose issuer.

use std::{fmt, sync::Arc};

use ori_domain::{EdgeId, FaceId};
use thiserror::Error;

use crate::{
    CandidateFaceTransform, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationResourceProfileV2, MaterialHingeGraphGeometry,
    MaterialHingeGraphInstanceV1,
};

mod validation;

use validation::{
    checkpoint_v2, decomposition_binding_with_checkpoint_v2, pose_binding_with_checkpoint_v2,
    transform_bits_v2,
};

/// Stable model identifier for the general-N pose provenance token.
pub const COMMON_ARTICULATION_POSE_MODEL_ID_V2: &str = "common_articulation_pose_authority_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const POSE_BASE_WORK_V2: usize = 16;
const POSE_AUTHORITY_BASE_BYTES_V2: usize = 512;
const POSE_BLOCK_RECORD_BYTES_V2: usize = 192;
const POSE_FACE_TRANSFORM_RECORD_BYTES_V2: usize = 128;
const POSE_HINGE_ANGLE_RECORD_BYTES_V2: usize = 32;
const POSE_ARTICULATION_FACE_RECORD_BYTES_V2: usize = 16;

/// Cooperative stop requested by a V2 pose operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationPoseStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// V2 pose issuance or revalidation failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationPoseErrorV2 {
    #[error("the general-N common-articulation pose input is malformed")]
    InvalidInput,
    #[error("the general-N common-articulation pose exceeds its resource profile")]
    ResourceLimit,
    #[error("the closed pose was not issued by the live parent geometry")]
    PoseIssuerMismatch,
    #[error("the decomposition was not issued by the live parent geometry")]
    DecompositionIssuerMismatch,
    #[error("the retained V2 pose authority does not match the live input")]
    IssuerMismatch,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the operation deadline elapsed")]
    DeadlineExceeded,
}

/// Live inputs for the general-N V2 pose issuer.
///
/// The only resource admission input is the immutable V2 resource profile.
#[derive(Clone, Copy)]
pub struct CommonArticulationPoseInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub paper_thickness_mm: f64,
    pub profile: &'a CommonArticulationResourceProfileV2,
}

/// Sealed, non-authorizing general-N pose provenance.
///
/// It has no V1 conversion, no `Deref`, and no persistence traits.
///
/// ```compile_fail
/// use ori_kinematics::{
///     CommonArticulationPoseAuthorityV1, CommonArticulationPoseAuthorityV2,
/// };
///
/// fn accepts_v1(_: CommonArticulationPoseAuthorityV1) {}
/// fn reject_v2(value: CommonArticulationPoseAuthorityV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationPoseAuthorityV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationPoseAuthorityV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationPoseAuthorityV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationPoseAuthorityV2>();
/// ```
pub struct CommonArticulationPoseAuthorityV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    issuer_pose: Arc<()>,
    decomposition_binding: [u8; 32],
    profile_binding: [u8; 32],
    paper_thickness_bits: u64,
    configured_max_blocks: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    submitted_faces: usize,
    submitted_hinges: usize,
    articulation_face_count: usize,
    blocks: Vec<CommonArticulationPoseBlockRestrictionV2>,
    articulation_faces: Vec<FaceId>,
    logical_work: usize,
    retained_bytes: usize,
    binding_fingerprint: [u8; 32],
}

impl fmt::Debug for CommonArticulationPoseAuthorityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommonArticulationPoseAuthorityV2")
            .field("model_id", &COMMON_ARTICULATION_POSE_MODEL_ID_V2)
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field("profile_binding", &self.profile_binding)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct CommonArticulationPoseBlockRestrictionV2 {
    geometry_issuer: MaterialHingeGraphInstanceV1,
    faces: Vec<FaceId>,
    face_transforms: Vec<CandidateFaceTransform>,
    hinge_angles: Vec<CommonArticulationHingeAngleBitsV2>,
    articulation_faces: Vec<FaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationHingeAngleBitsV2 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

impl CommonArticulationHingeAngleBitsV2 {
    #[must_use]
    pub const fn edge_v2(&self) -> EdgeId {
        self.edge
    }
    #[must_use]
    pub const fn angle_degrees_bits_v2(&self) -> u64 {
        self.angle_degrees_bits
    }
}

/// Read-only V2 block restriction retained by the pose provenance token.
pub struct CommonArticulationPoseBlockRestrictionRefV2<'a> {
    block: &'a CommonArticulationPoseBlockRestrictionV2,
}

impl CommonArticulationPoseBlockRestrictionRefV2<'_> {
    /// Revalidates that this read-only restriction remains tied to this exact
    /// issued block geometry, including its canonical face and hinge lists.
    #[must_use]
    pub fn is_for_geometry_v2(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
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
    pub fn face_ids_v2(&self) -> &[FaceId] {
        &self.block.faces
    }

    #[must_use]
    pub fn face_transforms_v2(&self) -> &[CandidateFaceTransform] {
        &self.block.face_transforms
    }

    #[must_use]
    pub fn articulation_faces_v2(&self) -> &[FaceId] {
        &self.block.articulation_faces
    }

    #[must_use]
    pub fn hinge_angles_v2(&self) -> &[CommonArticulationHingeAngleBitsV2] {
        &self.block.hinge_angles
    }
}

impl CommonArticulationPoseAuthorityV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_POSE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn configured_max_blocks_v2(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn retained_bytes_upper_bound_v2(&self) -> usize {
        self.retained_bytes
    }

    #[must_use]
    pub fn block_v2(
        &self,
        index: usize,
    ) -> Option<CommonArticulationPoseBlockRestrictionRefV2<'_>> {
        self.blocks
            .get(index)
            .map(|block| CommonArticulationPoseBlockRestrictionRefV2 { block })
    }

    #[must_use]
    pub fn articulation_faces_v2(&self) -> &[FaceId] {
        &self.articulation_faces
    }

    #[must_use]
    pub const fn profile_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.profile_binding
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationPoseInputV2<'_>,
    ) -> Result<(), CommonArticulationPoseErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationPoseInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
    ) -> Result<(), CommonArticulationPoseErrorV2> {
        let candidate =
            prove_common_articulation_pose_authority_with_checkpoint_v2(input, &mut checkpoint)?;
        checkpoint_v2(&mut checkpoint)?;
        let restrictions_equal =
            block_restrictions_equal_v2(&self.blocks, &candidate.blocks, &mut checkpoint)?;
        let articulation_faces_equal = face_slices_equal_with_checkpoint_v2(
            &self.articulation_faces,
            &candidate.articulation_faces,
            &mut checkpoint,
        )?;
        if self.issuer_geometry != candidate.issuer_geometry
            || !Arc::ptr_eq(&self.issuer_pose, &candidate.issuer_pose)
            || self.decomposition_binding != candidate.decomposition_binding
            || self.profile_binding != candidate.profile_binding
            || self.paper_thickness_bits != candidate.paper_thickness_bits
            || self.configured_max_blocks != candidate.configured_max_blocks
            || self.actual_block_count != candidate.actual_block_count
            || self.face_count != candidate.face_count
            || self.hinge_count != candidate.hinge_count
            || self.submitted_faces != candidate.submitted_faces
            || self.submitted_hinges != candidate.submitted_hinges
            || self.articulation_face_count != candidate.articulation_face_count
            || !restrictions_equal
            || !articulation_faces_equal
            || self.logical_work != candidate.logical_work
            || self.retained_bytes != candidate.retained_bytes
            || self.binding_fingerprint != candidate.binding_fingerprint
        {
            return Err(CommonArticulationPoseErrorV2::IssuerMismatch);
        }
        checkpoint_v2(&mut checkpoint)
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

    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
}

/// Proves a separately typed general-N pose provenance token.
pub fn prove_common_articulation_pose_authority_v2(
    input: CommonArticulationPoseInputV2<'_>,
) -> Result<CommonArticulationPoseAuthorityV2, CommonArticulationPoseErrorV2> {
    prove_common_articulation_pose_authority_with_checkpoint_v2(input, || Ok(()))
}

/// Proves the V2 pose token while observing cooperative stop checkpoints.
pub fn prove_common_articulation_pose_authority_with_checkpoint_v2(
    input: CommonArticulationPoseInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<CommonArticulationPoseAuthorityV2, CommonArticulationPoseErrorV2> {
    checkpoint_v2(&mut checkpoint)?;
    if !input.paper_thickness_mm.is_finite() || input.paper_thickness_mm <= 0.0 {
        return Err(CommonArticulationPoseErrorV2::InvalidInput);
    }
    if !input.pose.is_for_geometry(input.geometry) {
        return Err(CommonArticulationPoseErrorV2::PoseIssuerMismatch);
    }
    if !input.decomposition.is_for_geometry(input.geometry) {
        return Err(CommonArticulationPoseErrorV2::DecompositionIssuerMismatch);
    }
    if !input.decomposition.is_for_profile_v2(input.profile) {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }

    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
    {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }

    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let block_count = input.decomposition.blocks().len();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if block_count != actual_block_count
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.decomposition.face_count_v2() != face_count
        || input.decomposition.hinge_count_v2() != hinge_count
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || block_count > maximum.block_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
    {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }

    let mut submitted_faces = 0usize;
    let mut submitted_hinges = 0usize;
    for block in input.decomposition.blocks() {
        checkpoint_v2(&mut checkpoint)?;
        validate_canonical_miura_block_shape_v2(
            block.geometry().face_ids().len(),
            block.geometry().hinges().len(),
        )?;
        submitted_faces = submitted_faces
            .checked_add(block.geometry().face_ids().len())
            .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?;
        submitted_hinges = submitted_hinges
            .checked_add(block.geometry().hinges().len())
            .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?;
    }
    let articulation_face_count = input.decomposition.articulation_faces().len();
    if articulation_face_count
        != actual_block_count
            .checked_sub(1)
            .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?
    {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }
    let logical_work = pose_logical_work_v2(
        block_count,
        face_count,
        hinge_count,
        submitted_faces,
        submitted_hinges,
    )?;
    let retained_bytes = pose_retained_bytes_v2(
        block_count,
        submitted_faces,
        submitted_hinges,
        articulation_face_count,
    )?;
    if logical_work != actual.pose_logical_work_v2()
        || retained_bytes != actual.pose_retained_bytes_v2()
        || logical_work > maximum.pose_logical_work_v2()
        || retained_bytes > maximum.pose_retained_bytes_v2()
    {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }

    validate_parent_pose_shape_v2(input.geometry, input.pose, &mut checkpoint)?;
    let (blocks, articulation_faces) = build_block_restrictions_with_checkpoint_v2(
        input.pose,
        input.decomposition,
        &mut checkpoint,
    )?;
    let decomposition_binding =
        decomposition_binding_with_checkpoint_v2(input.decomposition, &mut checkpoint)?;
    let profile_binding = input.profile.binding_fingerprint_v2();
    let binding_fingerprint = pose_binding_with_checkpoint_v2(
        input,
        profile_binding,
        decomposition_binding,
        logical_work,
        retained_bytes,
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;

    Ok(CommonArticulationPoseAuthorityV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        issuer_pose: input.pose.instance_anchor_v2(),
        decomposition_binding,
        profile_binding,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        configured_max_blocks,
        actual_block_count,
        face_count,
        hinge_count,
        submitted_faces,
        submitted_hinges,
        articulation_face_count,
        blocks,
        articulation_faces,
        logical_work,
        retained_bytes,
        binding_fingerprint,
    })
}

fn pose_logical_work_v2(
    block_count: usize,
    face_count: usize,
    hinge_count: usize,
    submitted_faces: usize,
    submitted_hinges: usize,
) -> Result<usize, CommonArticulationPoseErrorV2> {
    let block_pairs = block_count
        .checked_mul(
            block_count
                .checked_sub(1)
                .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?;
    POSE_BASE_WORK_V2
        .checked_add(
            face_count
                .checked_mul(8)
                .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_add(hinge_count.checked_mul(12)?))
        .and_then(|value| value.checked_add(block_count.checked_mul(16)?))
        .and_then(|value| value.checked_add(submitted_faces.checked_mul(8)?))
        .and_then(|value| value.checked_add(submitted_hinges.checked_mul(12)?))
        .and_then(|value| value.checked_add(block_pairs.checked_mul(8)?))
        .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)
}

fn validate_canonical_miura_block_shape_v2(
    face_count: usize,
    hinge_count: usize,
) -> Result<(), CommonArticulationPoseErrorV2> {
    if face_count != 9 || hinge_count != 12 {
        return Err(CommonArticulationPoseErrorV2::ResourceLimit);
    }
    Ok(())
}

fn build_block_restrictions_with_checkpoint_v2(
    pose: &ClosedMaterialHingeGraphPose,
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<
    (Vec<CommonArticulationPoseBlockRestrictionV2>, Vec<FaceId>),
    CommonArticulationPoseErrorV2,
> {
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(decomposition.blocks().len())
        .map_err(|_| CommonArticulationPoseErrorV2::ResourceLimit)?;
    for block in decomposition.blocks() {
        checkpoint_v2(checkpoint)?;
        let mut face_transforms = Vec::new();
        face_transforms
            .try_reserve_exact(block.geometry().face_ids().len())
            .map_err(|_| CommonArticulationPoseErrorV2::ResourceLimit)?;
        for face in block.geometry().face_ids().iter().copied() {
            checkpoint_v2(checkpoint)?;
            let transform = pose
                .face_transform(face)
                .ok_or(CommonArticulationPoseErrorV2::InvalidInput)?;
            face_transforms.push(CandidateFaceTransform::new(face, transform));
        }

        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(block.geometry().hinges().len())
            .map_err(|_| CommonArticulationPoseErrorV2::ResourceLimit)?;
        for hinge in block.geometry().hinges() {
            checkpoint_v2(checkpoint)?;
            let angle_degrees_bits = pose
                .hinge_angles()
                .as_slice()
                .binary_search_by_key(&hinge.edge().canonical_bytes(), |angle| {
                    angle.edge().canonical_bytes()
                })
                .ok()
                .map(|index| {
                    pose.hinge_angles().as_slice()[index]
                        .angle_degrees()
                        .to_bits()
                })
                .ok_or(CommonArticulationPoseErrorV2::InvalidInput)?;
            hinge_angles.push(CommonArticulationHingeAngleBitsV2 {
                edge: hinge.edge(),
                angle_degrees_bits,
            });
        }
        hinge_angles.sort_unstable_by_key(|angle| angle.edge.canonical_bytes());

        let mut block_articulation_faces = Vec::new();
        for face in block.geometry().face_ids().iter().copied() {
            checkpoint_v2(checkpoint)?;
            if decomposition
                .articulation_faces()
                .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
                .is_ok()
            {
                block_articulation_faces.push(face);
            }
        }
        block_articulation_faces.sort_unstable_by_key(FaceId::canonical_bytes);
        blocks.push(CommonArticulationPoseBlockRestrictionV2 {
            geometry_issuer: block.geometry().instance_anchor_v1(),
            faces: block.geometry().face_ids().to_vec(),
            face_transforms,
            hinge_angles,
            articulation_faces: block_articulation_faces,
        });
    }
    let mut articulation_faces = Vec::new();
    articulation_faces
        .try_reserve_exact(decomposition.articulation_faces().len())
        .map_err(|_| CommonArticulationPoseErrorV2::ResourceLimit)?;
    for face in decomposition.articulation_faces() {
        checkpoint_v2(checkpoint)?;
        articulation_faces.push(*face);
    }
    if blocks.len() != decomposition.blocks().len() {
        return Err(CommonArticulationPoseErrorV2::InvalidInput);
    }
    for (restriction, source) in blocks.iter().zip(decomposition.blocks()) {
        checkpoint_v2(checkpoint)?;
        if !restriction.geometry_issuer.matches(source.geometry()) {
            return Err(CommonArticulationPoseErrorV2::InvalidInput);
        }
    }
    Ok((blocks, articulation_faces))
}

fn block_restrictions_equal_v2(
    expected: &[CommonArticulationPoseBlockRestrictionV2],
    actual: &[CommonArticulationPoseBlockRestrictionV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<bool, CommonArticulationPoseErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if expected.geometry_issuer != actual.geometry_issuer
            || expected.faces != actual.faces
            || expected.articulation_faces != actual.articulation_faces
            || expected.hinge_angles != actual.hinge_angles
            || expected.face_transforms.len() != actual.face_transforms.len()
        {
            return Ok(false);
        }
        for (expected_transform, actual_transform) in
            expected.face_transforms.iter().zip(&actual.face_transforms)
        {
            checkpoint_v2(checkpoint)?;
            if expected_transform.face() != actual_transform.face()
                || transform_bits_v2(expected_transform.transform())
                    != transform_bits_v2(actual_transform.transform())
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn face_slices_equal_with_checkpoint_v2(
    expected: &[FaceId],
    actual: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<bool, CommonArticulationPoseErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if expected != actual {
            return Ok(false);
        }
    }
    Ok(true)
}

fn pose_retained_bytes_v2(
    block_count: usize,
    submitted_faces: usize,
    submitted_hinges: usize,
    articulation_faces: usize,
) -> Result<usize, CommonArticulationPoseErrorV2> {
    POSE_AUTHORITY_BASE_BYTES_V2
        .checked_add(
            block_count
                .checked_mul(POSE_BLOCK_RECORD_BYTES_V2)
                .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)?,
        )
        .and_then(|value| {
            value.checked_add(submitted_faces.checked_mul(POSE_FACE_TRANSFORM_RECORD_BYTES_V2)?)
        })
        .and_then(|value| {
            value.checked_add(submitted_hinges.checked_mul(POSE_HINGE_ANGLE_RECORD_BYTES_V2)?)
        })
        .and_then(|value| {
            value.checked_add(
                articulation_faces.checked_mul(POSE_ARTICULATION_FACE_RECORD_BYTES_V2)?,
            )
        })
        .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)
}

fn validate_parent_pose_shape_v2(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<(), CommonArticulationPoseErrorV2> {
    if !geometry.face_ids().contains(&pose.fixed_face())
        || pose.transforms().len() != geometry.face_ids().len()
        || pose.hinge_angles().as_slice().len() != geometry.hinges().len()
        || pose.closure_certificate().checked_hinges().len() != geometry.hinges().len()
    {
        return Err(CommonArticulationPoseErrorV2::InvalidInput);
    }
    for (face, transform) in geometry.face_ids().iter().zip(pose.transforms()) {
        checkpoint_v2(checkpoint)?;
        if transform.face() != *face || !transform_is_finite_v2(transform.transform()) {
            return Err(CommonArticulationPoseErrorV2::InvalidInput);
        }
    }
    for (hinge, angle) in geometry.hinges().iter().zip(pose.hinge_angles().as_slice()) {
        checkpoint_v2(checkpoint)?;
        if hinge.edge() != angle.edge() || !angle.angle_degrees().is_finite() {
            return Err(CommonArticulationPoseErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn transform_is_finite_v2(transform: crate::RigidTransform) -> bool {
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

#[cfg(test)]
#[path = "common_articulation_pose_v2/tests.rs"]
mod tests;
