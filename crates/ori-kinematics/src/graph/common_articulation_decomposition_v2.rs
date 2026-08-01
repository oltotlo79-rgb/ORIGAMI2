//! Profile-bound, iterative general-N canonical edge-block decomposition.
//!
//! This module deliberately does not share the V1 issuer.  In particular the
//! V2 result is tied to the immutable resource-profile fingerprint and uses a
//! compact block geometry constructor, so no V2 result can be converted into
//! the independently bounded V1 decomposition authority.

use std::fmt;

use ori_domain::FaceId;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{CanonicalMaterialEdgeBlockV1, MaterialHingeGraphAudit};
use crate::{
    CommonArticulationResourceProfileV2, MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
    common_articulation_resource_profile::canonical_miura_decomposition_resources_v2,
};

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
pub(super) const CANONICAL_MIURA_FACES_PER_BLOCK_V2: usize = 9;
pub(super) const CANONICAL_MIURA_HINGES_PER_BLOCK_V2: usize = 12;
pub(super) const CANONICAL_MIURA_FACE_BOUNDARY_VERTICES_V2: usize = 4;
const CHECKPOINT_INTERVAL_V2: usize = 1_024;
pub(super) const UNASSIGNED_BLOCK_V2: usize = usize::MAX;
const COMMON_ARTICULATION_DECOMPOSITION_MODEL_ID_V2: &str =
    "common_articulation_edge_block_decomposition_v2";

/// Bounds retained by a V2 decomposition.  They are derived only from the
/// profile accepted by the issuer; callers cannot select a looser shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalEdgeBlockLimitsV2 {
    pub max_blocks: usize,
    pub max_faces_per_block: usize,
    pub max_hinges_per_block: usize,
}

/// Cooperative stop requested by a V2 decomposition operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDecompositionStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while issuing a profile-bound general-N decomposition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDecompositionErrorV2 {
    #[error("the general-N decomposition input is malformed or foreign")]
    InvalidInput,
    #[error("the general-N decomposition exceeds its resource profile")]
    ResourceLimit,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the operation deadline elapsed")]
    DeadlineExceeded,
}

/// Canonically ordered, profile-bound general-N edge blocks.
///
/// This value is deliberately neither cloneable nor serializable.  Its
/// retained block observations are not V1 decomposition authority and no V2
/// to V1 conversion exists.
///
/// ```compile_fail
/// use ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CanonicalMaterialEdgeBlockDecompositionV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::{
///     CanonicalMaterialEdgeBlockDecompositionV1, CanonicalMaterialEdgeBlockDecompositionV2,
/// };
///
/// fn accepts_v1(_: CanonicalMaterialEdgeBlockDecompositionV1) {}
/// fn reject_v2(value: CanonicalMaterialEdgeBlockDecompositionV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CanonicalMaterialEdgeBlockDecompositionV2>();
/// ```
pub struct CanonicalMaterialEdgeBlockDecompositionV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    profile_binding: [u8; 32],
    limits: CanonicalEdgeBlockLimitsV2,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    logical_work: usize,
    storage_bytes: usize,
    blocks: Vec<CanonicalMaterialEdgeBlockV1>,
    articulation_faces: Vec<FaceId>,
    binding_fingerprint: [u8; 32],
}

impl fmt::Debug for CanonicalMaterialEdgeBlockDecompositionV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalMaterialEdgeBlockDecompositionV2")
            .field("model_id", &COMMON_ARTICULATION_DECOMPOSITION_MODEL_ID_V2)
            .field("configured_max_blocks", &self.limits.max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field("profile_binding", &self.profile_binding)
            .finish_non_exhaustive()
    }
}

impl CanonicalMaterialEdgeBlockDecompositionV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DECOMPOSITION_MODEL_ID_V2
    }

    #[must_use]
    pub fn is_for_geometry(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer_geometry.matches(geometry)
    }

    /// The profile fingerprint must match exactly; a decomposition issued for
    /// configured N=33 cannot be reused under configured N=34 merely because
    /// the live graph happens to contain 33 blocks.
    #[must_use]
    pub fn is_for_profile_v2(&self, profile: &CommonArticulationResourceProfileV2) -> bool {
        self.profile_binding == profile.binding_fingerprint_v2()
    }

    #[must_use]
    pub const fn limits(&self) -> CanonicalEdgeBlockLimitsV2 {
        self.limits
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn face_count_v2(&self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn storage_bytes_upper_bound_v2(&self) -> usize {
        self.storage_bytes
    }

    #[must_use]
    pub const fn profile_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.profile_binding
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub fn blocks(&self) -> &[CanonicalMaterialEdgeBlockV1] {
        &self.blocks
    }

    #[must_use]
    pub fn articulation_faces(&self) -> &[FaceId] {
        &self.articulation_faces
    }
}

pub(super) struct WorkMeterV2 {
    consumed: usize,
    allowed: usize,
}

impl WorkMeterV2 {
    fn account(
        &mut self,
        units: usize,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
    ) -> Result<(), CommonArticulationDecompositionErrorV2> {
        let before_batch = self.consumed / CHECKPOINT_INTERVAL_V2;
        self.consumed = self
            .consumed
            .checked_add(units)
            .filter(|consumed| *consumed <= self.allowed)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        if self.consumed / CHECKPOINT_INTERVAL_V2 > before_batch {
            checkpoint_v2(checkpoint)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct RawBlockV2 {
    faces: Vec<FaceId>,
    hinge_indices: Vec<usize>,
}

/// Iterative Tarjan state.  Parent *edge* identity is retained separately
/// from the parent face so a parallel edge is handled as a back edge rather
/// than silently discarded.
#[derive(Debug, Clone, Copy)]
pub(super) struct TarjanFrameV2 {
    node: usize,
    next_neighbor: usize,
}

impl MaterialHingeGraphGeometry {
    /// Issues a general-N canonical decomposition under one immutable resource
    /// profile.  The profile binds both configured and actual N; live face and
    /// hinge counts, every canonical block shape, and the eventual result
    /// count must match it exactly.
    pub fn decompose_canonical_edge_blocks_with_profile_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        profile: &CommonArticulationResourceProfileV2,
    ) -> Result<CanonicalMaterialEdgeBlockDecompositionV2, CommonArticulationDecompositionErrorV2>
    {
        self.decompose_canonical_edge_blocks_with_checkpoint_v2(audit, profile, || Ok(()))
    }

    /// As [`Self::decompose_canonical_edge_blocks_with_profile_v2`], with a
    /// cooperative checkpoint at start, every 1,024 accounted records, and
    /// immediately before publishing the sealed result.
    pub fn decompose_canonical_edge_blocks_with_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        profile: &CommonArticulationResourceProfileV2,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
    ) -> Result<CanonicalMaterialEdgeBlockDecompositionV2, CommonArticulationDecompositionErrorV2>
    {
        checkpoint_v2(&mut checkpoint)?;
        let mut meter = WorkMeterV2 {
            consumed: 0,
            allowed: profile.actual_v2().decomposition_logical_work_v2(),
        };
        let (limits, logical_work, storage_bytes) =
            preflight_v2(self, audit, profile, &mut meter, &mut checkpoint)?;
        // `preflight_v2` proves the profile field used to seed this meter is
        // exactly this issuer's checked formula before any result allocation.
        meter.allowed = logical_work;

        let face_count = self.face_ids().len();
        let hinge_count = self.hinges().len();
        let actual_block_count = profile.actual_block_count_v2();
        let face_index = prepare_face_index_v2(self, &mut meter, &mut checkpoint)?;
        let adjacency =
            prepare_adjacency_v2(self, audit, &face_index, &mut meter, &mut checkpoint)?;
        let block_by_edge = tarjan_block_assignments_v2(
            &adjacency,
            hinge_count,
            actual_block_count,
            &mut meter,
            &mut checkpoint,
        )?;
        let mut raw = materialize_raw_blocks_v2(
            self,
            block_by_edge,
            actual_block_count,
            &mut meter,
            &mut checkpoint,
        )?;
        let order = canonical_raw_block_order_v2(self, &raw, &mut meter, &mut checkpoint)?;
        raw = reorder_raw_blocks_v2(raw, order, &mut meter, &mut checkpoint)?;
        let articulation_faces = articulation_faces_v2(
            &raw,
            self.face_ids(),
            actual_block_count,
            &mut meter,
            &mut checkpoint,
        )?;
        let blocks = materialize_blocks_v2(self, raw, &mut meter, &mut checkpoint)?;
        if blocks.len() != actual_block_count
            || blocks.len() != profile.actual_v2().block_count_v2()
            || articulation_faces.len()
                != actual_block_count
                    .checked_sub(1)
                    .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?
        {
            return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
        }
        let profile_binding = profile.binding_fingerprint_v2();
        let binding_fingerprint = decomposition_binding_v2(
            DecompositionBindingInputV2 {
                profile_binding,
                limits,
                face_count,
                hinge_count,
                blocks: &blocks,
                articulation_faces: &articulation_faces,
            },
            &mut meter,
            &mut checkpoint,
        )?;
        // A prepublication poll prevents a result becoming observable after a
        // caller's final cancellation/deadline request.
        checkpoint_v2(&mut checkpoint)?;

        Ok(CanonicalMaterialEdgeBlockDecompositionV2 {
            issuer_geometry: self.instance_anchor_v1(),
            profile_binding,
            limits,
            actual_block_count,
            face_count,
            hinge_count,
            logical_work,
            storage_bytes,
            blocks,
            articulation_faces,
            binding_fingerprint,
        })
    }
}

fn preflight_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    profile: &CommonArticulationResourceProfileV2,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<(CanonicalEdgeBlockLimitsV2, usize, usize), CommonArticulationDecompositionErrorV2> {
    let configured_max_blocks = profile.configured_max_blocks_v2();
    let actual_block_count = profile.actual_block_count_v2();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
    {
        return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
    }
    let actual = profile.actual_v2();
    let maximum = profile.maximum_v2();
    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    if face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || actual.block_count_v2() != actual_block_count
        || actual_block_count > maximum.block_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || geometry.face_ids().len() != audit.faces().len()
    {
        return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
    }
    let (expected_work, expected_storage) =
        canonical_miura_decomposition_resources_v2(actual_block_count)
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    let (maximum_work, maximum_storage) =
        canonical_miura_decomposition_resources_v2(configured_max_blocks)
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    if actual.decomposition_logical_work_v2() != expected_work
        || actual.decomposition_storage_bytes_v2() != expected_storage
        || maximum.decomposition_logical_work_v2() != maximum_work
        || maximum.decomposition_storage_bytes_v2() != maximum_storage
        || expected_work > maximum_work
        || expected_storage > maximum_storage
    {
        return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
    }
    for (geometry_face, audit_face) in geometry.face_ids().iter().zip(audit.faces()) {
        meter.account(1, checkpoint)?;
        if geometry_face != audit_face {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    for pair in geometry.face_ids().windows(2) {
        meter.account(1, checkpoint)?;
        if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    for pair in geometry.hinges().windows(2) {
        meter.account(1, checkpoint)?;
        if pair[0].edge().canonical_bytes() >= pair[1].edge().canonical_bytes() {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    // This condition is what turns the resource-profile's per-block compact
    // allocation term into a source-derived bound.  It also rejects a graph
    // with matching totals but non-Miura face payloads before allocation.
    for face in geometry.face_ids().iter().copied() {
        meter.account(1, checkpoint)?;
        if geometry
            .face_boundary_vertices(face)
            .is_none_or(|vertices| vertices.len() != CANONICAL_MIURA_FACE_BOUNDARY_VERTICES_V2)
        {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    Ok((
        CanonicalEdgeBlockLimitsV2 {
            max_blocks: configured_max_blocks,
            max_faces_per_block: CANONICAL_MIURA_FACES_PER_BLOCK_V2,
            max_hinges_per_block: CANONICAL_MIURA_HINGES_PER_BLOCK_V2,
        },
        expected_work,
        expected_storage,
    ))
}

mod tarjan;
use tarjan::{
    articulation_faces_v2, canonical_raw_block_order_v2, materialize_blocks_v2,
    materialize_raw_blocks_v2, prepare_adjacency_v2, prepare_face_index_v2, reorder_raw_blocks_v2,
    tarjan_block_assignments_v2,
};

struct DecompositionBindingInputV2<'a> {
    profile_binding: [u8; 32],
    limits: CanonicalEdgeBlockLimitsV2,
    face_count: usize,
    hinge_count: usize,
    blocks: &'a [CanonicalMaterialEdgeBlockV1],
    articulation_faces: &'a [FaceId],
}

fn decomposition_binding_v2(
    input: DecompositionBindingInputV2<'_>,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<[u8; 32], CommonArticulationDecompositionErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_DECOMPOSITION_MODEL_ID_V2.as_bytes());
    hash.update(input.profile_binding);
    for value in [
        input.limits.max_blocks,
        input.limits.max_faces_per_block,
        input.limits.max_hinges_per_block,
        input.face_count,
        input.hinge_count,
        input.blocks.len(),
        input.articulation_faces.len(),
    ] {
        hash_usize_v2(&mut hash, value)?;
    }
    for face in input.articulation_faces {
        meter.account(1, checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for block in input.blocks {
        meter.account(1, checkpoint)?;
        hash_usize_v2(&mut hash, block.geometry().face_ids().len())?;
        hash_usize_v2(&mut hash, block.geometry().hinges().len())?;
        for face in block.geometry().face_ids() {
            meter.account(1, checkpoint)?;
            hash.update(face.canonical_bytes());
        }
        for hinge in block.geometry().hinges() {
            meter.account(1, checkpoint)?;
            hash.update(hinge.edge().canonical_bytes());
            hash.update(hinge.left_face().canonical_bytes());
            hash.update(hinge.right_face().canonical_bytes());
        }
    }
    Ok(hash.finalize().into())
}

pub(super) fn reserved_zeros_v2(
    len: usize,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<usize>, CommonArticulationDecompositionErrorV2> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for _ in 0..len {
        meter.account(1, checkpoint)?;
        values.push(0);
    }
    Ok(values)
}

pub(super) fn reserved_options_v2(
    len: usize,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<Option<usize>>, CommonArticulationDecompositionErrorV2> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for _ in 0..len {
        meter.account(1, checkpoint)?;
        values.push(None);
    }
    Ok(values)
}

fn hash_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationDecompositionErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<(), CommonArticulationDecompositionErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDecompositionStopV2::Cancelled => {
            CommonArticulationDecompositionErrorV2::Cancelled
        }
        CommonArticulationDecompositionStopV2::DeadlineExceeded => {
            CommonArticulationDecompositionErrorV2::DeadlineExceeded
        }
    })
}
