use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId};
use ori_foldability::LayerOrderSnapshot;
pub use ori_kinematics::{
    COMMON_ARTICULATION_POSE_MAX_BLOCKS_V1, COMMON_ARTICULATION_POSE_MIN_BLOCKS_V1,
    COMMON_ARTICULATION_POSE_MODEL_ID_V1, CommonArticulationHingeAngleBitsV1,
    CommonArticulationPoseAuthorityV1, CommonArticulationPoseBlockRestrictionRefV1,
    CommonArticulationPoseErrorV1, CommonArticulationPoseInputV1, CommonArticulationPoseLimitsV1,
    CommonArticulationPoseStopV1,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV1, CycleScheduleLimitsV1,
    CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, prove_common_articulation_pose_authority_with_checkpoint_v1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CommonArticulationClearanceErrorV1, CommonArticulationClearanceLimitsV1,
    CommonArticulationClearancePrerequisiteV1, CommonArticulationClearanceRevalidationInputV1,
    CooperativeOperationControlV1, CooperativeOperationStopV1,
    GeneralMultiFaceCellTransportProofV1, PositiveThicknessContinuousCertificateV1,
};

pub const BLOCK_COMPOSED_PATH_MODEL_ID_V1: &str = "block_composed_path_authority_v1";
pub const COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_MODEL_ID_V1: &str =
    "common_articulation_block_composed_path_authority_v1";
pub const COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1: &str =
    "common_articulation_continuous_layer_path_authority_v1";
pub const BLOCK_COMPOSITION_LIMIT_V1: usize = 32;
pub const BLOCKWISE_CLOSURE_MODEL_ID_V1: &str = "blockwise_interval_closure_authority_v1";
pub const BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1: &str = "blockwise_positive_layer_authority_v1";
pub const BLOCKWISE_POSITIVE_LAYER_ARITY_V1: usize = 2;
pub const MULTI_BLOCK_MIN_BLOCKS_V1: usize = 2;
pub const MULTI_BLOCK_MAX_BLOCKS_V1: usize = 8;
const EXACT_NINE_BLOCK_ARITY_V1: usize = 9;
const EXACT_TEN_BLOCK_ARITY_V1: usize = 10;
pub const MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1: &str =
    "bounded_multi_block_positive_layer_authority_v1";
pub const COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1: &str =
    "complete_live_multi_block_positive_layer_authority_v1";
pub const BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MultiBlockAdmissionScopeV1 {
    GenericSubmitted2To8,
    ExactNineSubmittedSet,
    ExactTenSubmittedSet,
}

impl MultiBlockAdmissionScopeV1 {
    const fn admits_block_count_v1(self, count: usize) -> bool {
        match self {
            Self::GenericSubmitted2To8 => multi_block_count_supported_v1(count),
            Self::ExactNineSubmittedSet => count == EXACT_NINE_BLOCK_ARITY_V1,
            Self::ExactTenSubmittedSet => count == EXACT_TEN_BLOCK_ARITY_V1,
        }
    }

    const fn closure_domain_tag_v1(self) -> &'static [u8] {
        match self {
            Self::GenericSubmitted2To8 => b"closure_v1",
            Self::ExactNineSubmittedSet => b"exact-nine-submitted-set-closure-v1",
            Self::ExactTenSubmittedSet => b"exact-ten-submitted-set-closure-v1",
        }
    }

    const fn positive_layer_domain_tag_v1(self) -> Option<&'static [u8]> {
        match self {
            Self::GenericSubmitted2To8 => None,
            Self::ExactNineSubmittedSet => Some(b"exact-nine-submitted-set-positive-layer-v1"),
            Self::ExactTenSubmittedSet => Some(b"exact-ten-submitted-set-positive-layer-v1"),
        }
    }

    const fn complete_live_domain_tag_v1(self) -> Option<&'static [u8]> {
        match self {
            Self::GenericSubmitted2To8 => None,
            Self::ExactNineSubmittedSet => Some(b"exact-nine-submitted-set-complete-live-v1"),
            Self::ExactTenSubmittedSet => Some(b"exact-ten-submitted-set-complete-live-v1"),
        }
    }
}

pub fn issue_common_articulation_pose_authority_v1(
    input: CommonArticulationPoseInputV1<'_>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    issue_common_articulation_pose_authority_with_control_v1(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_pose_authority_with_control_v1(
    input: CommonArticulationPoseInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1> {
    prove_common_articulation_pose_authority_with_checkpoint_v1(input, || {
        control.checkpoint().map_err(|stop| match stop {
            CooperativeOperationStopV1::Cancelled => CommonArticulationPoseStopV1::Cancelled,
            CooperativeOperationStopV1::DeadlineExceeded => {
                CommonArticulationPoseStopV1::DeadlineExceeded
            }
        })
    })
}

pub struct BlockUnionCompletenessInputV1<'a> {
    pub faces: &'a [FaceId],
    pub hinges: &'a [EdgeId],
}

#[derive(Debug, Clone)]
pub struct BlockUnionCompletenessGapReportV1 {
    scope: MultiBlockAdmissionScopeV1,
    issuer: MaterialHingeGraphGeometry,
    live_faces: Vec<FaceId>,
    live_hinges: Vec<EdgeId>,
    submitted_faces: Vec<FaceId>,
    submitted_hinges: Vec<EdgeId>,
    blocks: Vec<CanonicalBlockBindingV1>,
    complete: bool,
}

impl BlockUnionCompletenessGapReportV1 {
    #[must_use]
    pub fn live_faces(&self) -> &[FaceId] {
        &self.live_faces
    }
    #[must_use]
    pub fn live_hinges(&self) -> &[EdgeId] {
        &self.live_hinges
    }
    #[must_use]
    pub fn submitted_faces(&self) -> &[FaceId] {
        &self.submitted_faces
    }
    #[must_use]
    pub fn submitted_hinges(&self) -> &[EdgeId] {
        &self.submitted_hinges
    }
    #[must_use]
    pub const fn exact_live_union_observed(&self) -> bool {
        self.complete
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_multi_block_composition(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer.same_instance(geometry)
            && self.live_faces == canonical_faces_v1(geometry)
            && self.live_hinges == canonical_hinges_v1(geometry)
    }
}

fn canonical_faces_v1(geometry: &MaterialHingeGraphGeometry) -> Vec<FaceId> {
    let mut ids = geometry.face_ids().to_vec();
    ids.sort_unstable_by_key(FaceId::canonical_bytes);
    ids
}

fn canonical_hinges_v1(geometry: &MaterialHingeGraphGeometry) -> Vec<EdgeId> {
    let mut ids = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    ids
}

fn canonical_faces_with_checkpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<Vec<FaceId>, CommonArticulationContinuousLayerPathErrorV1> {
    let mut ids = Vec::new();
    ids.try_reserve_exact(geometry.face_ids().len())
        .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
    for face in geometry.face_ids() {
        checkpoint()?;
        ids.push(*face);
    }
    ids.sort_unstable_by_key(FaceId::canonical_bytes);
    checkpoint()?;
    Ok(ids)
}

fn canonical_hinges_with_checkpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<Vec<EdgeId>, CommonArticulationContinuousLayerPathErrorV1> {
    let mut ids = Vec::new();
    ids.try_reserve_exact(geometry.hinges().len())
        .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
    for hinge in geometry.hinges() {
        checkpoint()?;
        ids.push(hinge.edge());
    }
    ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    checkpoint()?;
    Ok(ids)
}

#[must_use]
pub fn diagnose_block_union_completeness_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: &[BlockUnionCompletenessInputV1<'_>],
) -> Option<BlockUnionCompletenessGapReportV1> {
    diagnose_block_union_completeness_with_scope_v1(
        geometry,
        blocks,
        MultiBlockAdmissionScopeV1::GenericSubmitted2To8,
    )
}

/// Exact-nine companion for one bounded submitted block set.
///
/// The generic submitted-set authority remains frozen at 2..=8. This entry
/// admits only nine inputs and still returns non-authorizing union evidence;
/// the caller must independently bind the exact live graph and every motion
/// theorem before it can reach Apply.
#[must_use]
pub fn diagnose_exact_nine_block_union_completeness_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: &[BlockUnionCompletenessInputV1<'_>],
) -> Option<BlockUnionCompletenessGapReportV1> {
    diagnose_block_union_completeness_with_scope_v1(
        geometry,
        blocks,
        MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
    )
}

/// Exact-ten companion for one bounded submitted block set.
///
/// The generic submitted-set authority remains frozen at 2..=8 and the
/// exact-nine scope remains frozen at nine. This entry admits only ten inputs
/// and returns the same non-authorizing union evidence under a distinct domain.
#[must_use]
pub fn diagnose_exact_ten_block_union_completeness_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: &[BlockUnionCompletenessInputV1<'_>],
) -> Option<BlockUnionCompletenessGapReportV1> {
    diagnose_block_union_completeness_with_scope_v1(
        geometry,
        blocks,
        MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
    )
}

fn diagnose_block_union_completeness_with_scope_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: &[BlockUnionCompletenessInputV1<'_>],
    scope: MultiBlockAdmissionScopeV1,
) -> Option<BlockUnionCompletenessGapReportV1> {
    let live_item_count = geometry
        .face_ids()
        .len()
        .checked_add(geometry.hinges().len())?;
    if !scope.admits_block_count_v1(blocks.len())
        || blocks
            .iter()
            .any(|block| block.faces.is_empty() || block.hinges.is_empty())
        || live_item_count > BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1
    {
        return None;
    }
    let live_faces = canonical_faces_v1(geometry);
    let live_hinges = canonical_hinges_v1(geometry);
    let submitted_count = blocks.iter().try_fold(0usize, |count, block| {
        count
            .checked_add(block.faces.len())?
            .checked_add(block.hinges.len())
    })?;
    if submitted_count > BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1 {
        return None;
    }
    let mut canonical_blocks = Vec::with_capacity(blocks.len());
    for block in blocks {
        let mut faces = block.faces.to_vec();
        let mut edges = block.hinges.to_vec();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if faces.windows(2).any(|pair| pair[0] == pair[1])
            || edges.windows(2).any(|pair| pair[0] == pair[1])
        {
            return None;
        }
        canonical_blocks.push(CanonicalBlockBindingV1 { edges, faces });
    }
    canonical_blocks.sort_unstable_by_key(|block| block.edges[0].canonical_bytes());
    let mut submitted_faces = blocks
        .iter()
        .flat_map(|block| block.faces.iter().copied())
        .collect::<Vec<_>>();
    let mut submitted_hinges = blocks
        .iter()
        .flat_map(|block| block.hinges.iter().copied())
        .collect::<Vec<_>>();
    submitted_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    submitted_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut face_union = submitted_faces.clone();
    face_union.dedup();
    let hinge_duplicates = submitted_hinges.windows(2).any(|pair| pair[0] == pair[1]);
    let complete = face_union == live_faces
        && submitted_hinges == live_hinges
        && !hinge_duplicates
        && block_articulation_incidence_is_tree_v1(&canonical_blocks);
    Some(BlockUnionCompletenessGapReportV1 {
        scope,
        issuer: geometry.clone(),
        live_faces,
        live_hinges,
        submitted_faces,
        submitted_hinges,
        blocks: canonical_blocks,
        complete,
    })
}

#[must_use]
pub const fn multi_block_count_supported_v1(count: usize) -> bool {
    count >= MULTI_BLOCK_MIN_BLOCKS_V1 && count <= MULTI_BLOCK_MAX_BLOCKS_V1
}

/// One member of a caller-supplied bounded block set.
///
/// This input does not identify a containing project graph. Consequently the
/// issuer can prove only the submitted blocks' tree composition; it cannot
/// prove that their hinge union is a complete partition of a larger graph.
pub struct MultiBlockClosureInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
}

struct OwnedMultiBlockV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
}

/// Sealed authority for one submitted tree.
///
/// The generic issuer remains frozen at 2..=8 blocks; a separately scoped
/// companions admit exactly nine or exactly ten. No branch is whole-graph or project-
/// mutation authority. A production adapter must separately bind the canonical
/// union of all submitted hinges to the complete live graph.
pub struct MultiBlockClosureAuthorityV1 {
    scope: MultiBlockAdmissionScopeV1,
    binding: [u8; 32],
    blocks: Vec<OwnedMultiBlockV1>,
    thickness_bits: u64,
    issuer_context: [u8; 32],
}

impl MultiBlockClosureAuthorityV1 {
    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.blocks.len()
    }
}

pub struct MultiBlockPositiveLayerInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub source: &'a LayerOrderSnapshot,
    pub positive: PositiveThicknessContinuousCertificateV1,
    pub layer: GeneralMultiFaceCellTransportProofV1,
}

/// Positive-thickness and layer authority for the same bounded submitted set.
///
/// Revalidation binds every owned per-block proof and source snapshot, but it
/// does not add the missing whole-graph completeness premise described by
/// [`MultiBlockClosureAuthorityV1`].
pub struct MultiBlockPositiveLayerAuthorityV1 {
    scope: MultiBlockAdmissionScopeV1,
    binding: [u8; 32],
    parent: MultiBlockClosureAuthorityV1,
    positive: Vec<PositiveThicknessContinuousCertificateV1>,
    layer: Vec<GeneralMultiFaceCellTransportProofV1>,
    articulation_layer_fingerprint: [u8; 32],
}

/// Sealed evidence that one bounded submitted multi-block authority covers the
/// exact live geometry instance from which its gap report was issued.
///
/// This type closes only the submitted-set completeness boundary. It does not
/// prove cross-block continuous clearance, a common articulation pose, or
/// cross-block layer transport, and therefore never authorizes Apply, project
/// mutation, or a viewer snapshot.
pub struct CompleteMultiBlockPositiveLayerAuthorityV1 {
    scope: MultiBlockAdmissionScopeV1,
    binding: [u8; 32],
    issuer: MaterialHingeGraphGeometry,
    live_faces: Vec<FaceId>,
    live_hinges: Vec<EdgeId>,
    blocks: Vec<CanonicalBlockBindingV1>,
    parent: MultiBlockPositiveLayerAuthorityV1,
}

impl CompleteMultiBlockPositiveLayerAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.blocks.len()
    }

    #[cfg(test)]
    pub(crate) fn corrupt_scope_for_test_v1(&mut self) {
        self.scope = match self.scope {
            MultiBlockAdmissionScopeV1::GenericSubmitted2To8 => {
                MultiBlockAdmissionScopeV1::ExactNineSubmittedSet
            }
            MultiBlockAdmissionScopeV1::ExactNineSubmittedSet => {
                MultiBlockAdmissionScopeV1::ExactTenSubmittedSet
            }
            MultiBlockAdmissionScopeV1::ExactTenSubmittedSet => {
                MultiBlockAdmissionScopeV1::ExactNineSubmittedSet
            }
        };
    }

    #[cfg(test)]
    pub(crate) fn corrupt_partition_for_test_v1(&mut self) {
        self.blocks.swap(0, 1);
    }

    #[must_use]
    pub const fn exact_live_union_certified_v1(&self) -> bool {
        true
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
    #[allow(clippy::too_many_arguments)]
    pub fn revalidates_v1(
        &self,
        live_geometry: &MaterialHingeGraphGeometry,
        sources: &[&LayerOrderSnapshot],
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
        target_angles: &[(EdgeId, f64)],
    ) -> bool {
        self.revalidates_with_checkpoint_v1(
            live_geometry,
            sources,
            thickness,
            issuer_context,
            articulation_layer_fingerprint,
            target_angles,
            &mut || Ok(()),
        )
        .unwrap_or(false)
    }

    #[allow(clippy::too_many_arguments)]
    fn revalidates_with_checkpoint_v1(
        &self,
        live_geometry: &MaterialHingeGraphGeometry,
        sources: &[&LayerOrderSnapshot],
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
        target_angles: &[(EdgeId, f64)],
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
    ) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
        checkpoint()?;
        if !self.issuer.same_instance(live_geometry) {
            return Ok(false);
        }
        let live_faces = canonical_faces_with_checkpoint_v1(live_geometry, checkpoint)?;
        let live_hinges = canonical_hinges_with_checkpoint_v1(live_geometry, checkpoint)?;
        if self.scope != self.parent.scope
            || self.parent.scope != self.parent.parent.scope
            || !self.scope.admits_block_count_v1(self.blocks.len())
            || self.live_faces != live_faces
            || self.live_hinges != live_hinges
            || !complete_block_union_matches_live_with_checkpoint_v1(
                self.scope,
                &self.blocks,
                &self.live_faces,
                &self.live_hinges,
                checkpoint,
            )?
            || owned_multi_block_bindings_with_checkpoint_v1(&self.parent, checkpoint)?
                != self.blocks
            || !self.parent.revalidates_with_checkpoint_v1(
                sources,
                thickness,
                issuer_context,
                articulation_layer_fingerprint,
                checkpoint,
            )?
            || !self
                .parent
                .target_angles_match_with_checkpoint_v1(target_angles, checkpoint)?
            || complete_multi_block_positive_layer_binding_with_checkpoint_v1(
                self.scope,
                self.parent.binding,
                &self.live_faces,
                &self.live_hinges,
                &self.blocks,
                checkpoint,
            )? != self.binding
        {
            return Ok(false);
        }
        checkpoint()?;
        Ok(true)
    }
}

impl MultiBlockPositiveLayerAuthorityV1 {
    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.parent.blocks.len()
    }

    #[must_use]
    pub fn transition_count_v1(&self) -> usize {
        self.layer
            .iter()
            .map(|proof| proof.transition_hashes().len())
            .sum()
    }

    #[must_use]
    pub fn pair_order_count_v1(&self) -> usize {
        self.layer
            .iter()
            .map(|proof| proof.pair_order_count())
            .sum()
    }

    #[must_use]
    pub fn target_order_hash_v1(&self) -> [u8; 32] {
        let mut targets = self
            .layer
            .iter()
            .map(GeneralMultiFaceCellTransportProofV1::target_order_hash)
            .collect::<Vec<_>>();
        targets.sort_unstable();
        let mut hash = Sha256::new();
        hash.update(MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
        hash.update(b"target_order_v1");
        for target in targets {
            hash.update(target);
        }
        hash.finalize().into()
    }

    #[must_use]
    pub fn target_angles_match_v1(&self, actual: &[(EdgeId, f64)]) -> bool {
        self.target_angles_match_with_checkpoint_v1(actual, &mut || Ok(()))
            .unwrap_or(false)
    }

    fn target_angles_match_with_checkpoint_v1(
        &self,
        actual: &[(EdgeId, f64)],
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
    ) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
        let mut expected = Vec::new();
        for block in &self.parent.blocks {
            checkpoint()?;
            let Some(endpoint) = block.schedule.evaluate(1.0) else {
                return Ok(false);
            };
            for angle in endpoint.as_slice() {
                checkpoint()?;
                expected.push((angle.edge(), angle.angle_degrees()));
            }
        }
        expected.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        for pair in expected.windows(2) {
            checkpoint()?;
            if pair[0].0 == pair[1].0 {
                return Ok(false);
            }
        }
        let mut canonical_actual = Vec::new();
        for angle in actual {
            checkpoint()?;
            canonical_actual.push(*angle);
        }
        canonical_actual.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        if expected.len() != canonical_actual.len() {
            return Ok(false);
        }
        for (expected, actual) in expected.iter().zip(canonical_actual) {
            checkpoint()?;
            if expected.0 != actual.0 || expected.1.to_bits() != actual.1.to_bits() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[must_use]
    pub fn revalidates_v1(
        &self,
        sources: &[&LayerOrderSnapshot],
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
    ) -> bool {
        self.revalidates_with_checkpoint_v1(
            sources,
            thickness,
            issuer_context,
            articulation_layer_fingerprint,
            &mut || Ok(()),
        )
        .unwrap_or(false)
    }

    fn revalidates_with_checkpoint_v1(
        &self,
        sources: &[&LayerOrderSnapshot],
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
    ) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
        checkpoint()?;
        if self.scope != self.parent.scope
            || !self.scope.admits_block_count_v1(self.parent.blocks.len())
            || sources.len() != self.parent.blocks.len()
            || thickness.to_bits() != self.parent.thickness_bits
            || issuer_context != self.parent.issuer_context
            || issuer_context == [0; 32]
            || articulation_layer_fingerprint != self.articulation_layer_fingerprint
            || articulation_layer_fingerprint == [0; 32]
        {
            return Ok(false);
        }
        for (index, source) in sources.iter().enumerate() {
            checkpoint()?;
            let block = &self.parent.blocks[index];
            let fixed_face = block.closure.fixed_face();
            if !self.positive[index].is_for(
                &block.geometry,
                &block.audit,
                fixed_face,
                &block.schedule,
                &block.closure,
                thickness,
            ) {
                return Ok(false);
            }
            checkpoint()?;
            if !layer_proof_is_for_with_final_checkpoint_v1(
                &self.layer[index],
                &block.geometry,
                source,
                &block.schedule,
                &block.closure,
                thickness,
                checkpoint,
            )? {
                return Ok(false);
            }
        }
        Ok(multi_block_positive_layer_binding_with_checkpoint_v1(
            self.scope,
            self.parent.binding,
            &self.layer,
            articulation_layer_fingerprint,
            checkpoint,
        )? == self.binding)
    }
}

pub struct BlockwiseClosureInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
}

pub struct BlockwiseClosureAuthorityV1 {
    binding: [u8; 32],
    blocks: [(
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        CanonicalCycleScheduleV1,
        DyadicMaterialHingeIntervalClosureCertificateV1,
    ); 2],
    articulation: FaceId,
    thickness_bits: u64,
}

impl BlockwiseClosureAuthorityV1 {
    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn revalidates_v1(
        &self,
        articulation: FaceId,
        thickness: f64,
        issuer_context: [u8; 32],
    ) -> bool {
        articulation == self.articulation
            && thickness.to_bits() == self.thickness_bits
            && issuer_context != [0; 32]
            && {
                let refs = self
                    .blocks
                    .each_ref()
                    .map(|(geometry, _, schedule, closure)| (geometry, schedule, closure));
                blockwise_binding_v1(&refs, articulation, thickness, issuer_context) == self.binding
            }
    }
}

pub struct BlockwisePositiveLayerInputV1<'a> {
    pub source: &'a LayerOrderSnapshot,
    pub positive: PositiveThicknessContinuousCertificateV1,
    pub layer: GeneralMultiFaceCellTransportProofV1,
}

/// Opaque authority proving that both sides of a two-block articulation have
/// independently retained positive thickness and transported their native
/// layer orders over the exact closure owned by the parent authority.
pub struct BlockwisePositiveLayerAuthorityV1 {
    binding: [u8; 32],
    parent: BlockwiseClosureAuthorityV1,
    positive: [PositiveThicknessContinuousCertificateV1; 2],
    layer: [GeneralMultiFaceCellTransportProofV1; 2],
    articulation_layer_fingerprint: [u8; 32],
}

impl BlockwisePositiveLayerAuthorityV1 {
    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn transition_count_v1(&self) -> usize {
        self.layer
            .iter()
            .map(|proof| proof.transition_hashes().len())
            .sum()
    }

    #[must_use]
    pub fn pair_order_count_v1(&self) -> usize {
        self.layer
            .iter()
            .map(|proof| proof.pair_order_count())
            .sum()
    }

    #[must_use]
    pub fn target_order_hash_v1(&self) -> [u8; 32] {
        let mut targets = self
            .layer
            .iter()
            .map(GeneralMultiFaceCellTransportProofV1::target_order_hash)
            .collect::<Vec<_>>();
        targets.sort_unstable();
        let mut hash = Sha256::new();
        hash.update(BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
        hash.update(b"target_order_v1");
        for target in targets {
            hash.update(target);
        }
        hash.finalize().into()
    }

    #[must_use]
    pub fn target_angles_match_v1(&self, actual: &[(EdgeId, f64)]) -> bool {
        let mut expected = Vec::new();
        for (_, _, schedule, _) in &self.parent.blocks {
            let Some(endpoint) = schedule.evaluate(1.0) else {
                return false;
            };
            expected.extend(
                endpoint
                    .as_slice()
                    .iter()
                    .map(|angle| (angle.edge(), angle.angle_degrees())),
            );
        }
        expected.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        if expected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return false;
        }
        let mut actual = actual.to_vec();
        actual.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        expected.len() == actual.len()
            && expected.iter().zip(actual).all(|(expected, actual)| {
                expected.0 == actual.0 && expected.1.to_bits() == actual.1.to_bits()
            })
    }

    #[must_use]
    pub fn revalidates_v1(
        &self,
        sources: [&LayerOrderSnapshot; 2],
        articulation: FaceId,
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
    ) -> bool {
        if articulation_layer_fingerprint != self.articulation_layer_fingerprint
            || articulation_layer_fingerprint == [0; 32]
            || !self
                .parent
                .revalidates_v1(articulation, thickness, issuer_context)
        {
            return false;
        }
        for (index, source) in sources.into_iter().enumerate() {
            let (geometry, audit, schedule, closure) = &self.parent.blocks[index];
            if !self.positive[index].is_for(
                geometry,
                audit,
                articulation,
                schedule,
                closure,
                thickness,
            ) || !self.layer[index].is_for(geometry, source, schedule, closure, thickness)
            {
                return false;
            }
        }
        blockwise_positive_layer_binding_v1(
            self.parent.binding,
            &self.layer,
            articulation_layer_fingerprint,
        ) == self.binding
    }
}

fn blockwise_positive_layer_binding_v1(
    parent_binding: [u8; 32],
    layers: &[GeneralMultiFaceCellTransportProofV1; 2],
    articulation_layer_fingerprint: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    hash.update(parent_binding);
    hash.update(articulation_layer_fingerprint);
    let mut records = layers
        .iter()
        .map(|layer| {
            (
                layer.target_order_hash(),
                layer.paper_thickness_mm().to_bits(),
                layer.pair_order_count(),
            )
        })
        .collect::<Vec<_>>();
    records.sort_unstable();
    for (target, thickness, pair_count) in records {
        hash.update(target);
        hash.update(thickness.to_le_bytes());
        hash.update((pair_count as u64).to_le_bytes());
    }
    hash.finalize().into()
}

pub fn issue_blockwise_positive_layer_authority_v1(
    parent: BlockwiseClosureAuthorityV1,
    inputs: [BlockwisePositiveLayerInputV1<'_>; 2],
    articulation: FaceId,
    thickness: f64,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
) -> Option<BlockwisePositiveLayerAuthorityV1> {
    if articulation_layer_fingerprint == [0; 32]
        || !parent.revalidates_v1(articulation, thickness, issuer_context)
    {
        return None;
    }
    for (index, input) in inputs.iter().enumerate() {
        let (geometry, audit, schedule, closure) = &parent.blocks[index];
        if !input
            .positive
            .is_for(geometry, audit, articulation, schedule, closure, thickness)
            || !input
                .layer
                .is_for(geometry, input.source, schedule, closure, thickness)
        {
            return None;
        }
    }
    let [first, second] = inputs;
    let positive = [first.positive, second.positive];
    let layer = [first.layer, second.layer];
    let binding =
        blockwise_positive_layer_binding_v1(parent.binding, &layer, articulation_layer_fingerprint);
    Some(BlockwisePositiveLayerAuthorityV1 {
        binding,
        parent,
        positive,
        layer,
        articulation_layer_fingerprint,
    })
}

fn blockwise_binding_v1(
    blocks: &[(
        &MaterialHingeGraphGeometry,
        &CanonicalCycleScheduleV1,
        &DyadicMaterialHingeIntervalClosureCertificateV1,
    ); 2],
    articulation: FaceId,
    thickness: f64,
    issuer_context: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(BLOCKWISE_CLOSURE_MODEL_ID_V1.as_bytes());
    hash.update(articulation.canonical_bytes());
    hash.update(thickness.to_bits().to_le_bytes());
    hash.update(issuer_context);
    let mut records = blocks
        .iter()
        .map(|(geometry, schedule, closure)| {
            (
                schedule.graph_binding_fingerprint_v1(),
                schedule.certificate_binding_fingerprint_v2(),
                closure.partition_binding_fingerprint_v2(),
                geometry.hinges().len(),
                geometry.face_ids().len(),
            )
        })
        .collect::<Vec<_>>();
    records.sort_unstable_by_key(|record| record.0);
    for (graph, schedule, closure, edges, faces) in records {
        hash.update(graph);
        hash.update(schedule);
        hash.update(closure);
        hash.update((edges as u64).to_le_bytes());
        hash.update((faces as u64).to_le_bytes());
    }
    hash.finalize().into()
}

pub fn issue_blockwise_closure_authority_v1(
    inputs: [BlockwiseClosureInputV1<'_>; 2],
    articulation: FaceId,
    thickness: f64,
    issuer_context: [u8; 32],
) -> Option<BlockwiseClosureAuthorityV1> {
    if !thickness.is_finite() || thickness <= 0.0 || issuer_context == [0; 32] {
        return None;
    }
    let mut edge_sets = Vec::with_capacity(2);
    let mut face_sets = Vec::with_capacity(2);
    for input in &inputs {
        if !input
            .schedule
            .matches_binding(input.geometry, input.audit, articulation)
            || input.closure.fixed_face() != articulation
            || !input.closure.every_leaf_covers_graph_v1(input.geometry)
            || input.schedule.evaluate(0.0).is_none()
            || input.schedule.evaluate(1.0).is_none()
        {
            return None;
        }
        edge_sets.push(
            input
                .geometry
                .hinges()
                .iter()
                .map(|hinge| hinge.edge())
                .collect::<HashSet<_>>(),
        );
        face_sets.push(
            input
                .geometry
                .face_ids()
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        );
    }
    if !edge_sets[0].is_disjoint(&edge_sets[1])
        || face_sets[0]
            .intersection(&face_sets[1])
            .copied()
            .collect::<HashSet<_>>()
            != HashSet::from([articulation])
    {
        return None;
    }
    let refs = inputs
        .each_ref()
        .map(|input| (input.geometry, input.schedule, input.closure));
    let binding = blockwise_binding_v1(&refs, articulation, thickness, issuer_context);
    let blocks = inputs.map(|input| {
        (
            input.geometry.clone(),
            input.audit.clone(),
            input.schedule.clone(),
            input.closure.clone(),
        )
    });
    Some(BlockwiseClosureAuthorityV1 {
        binding,
        blocks,
        articulation,
        thickness_bits: thickness.to_bits(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalBlockBindingV1 {
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
}

/// Validates the bipartite block-articulation incidence graph.
///
/// A face shared by three or more blocks is one articulation node with a
/// star of incidences.  The pairwise block projection would turn that valid
/// star into a clique and falsely reject it as cyclic.
fn block_articulation_incidence_is_tree_v1(blocks: &[CanonicalBlockBindingV1]) -> bool {
    if blocks.len() < 2 {
        return false;
    }
    let mut occurrences = blocks
        .iter()
        .enumerate()
        .flat_map(|(block, binding)| binding.faces.iter().copied().map(move |face| (face, block)))
        .collect::<Vec<_>>();
    occurrences.sort_unstable_by_key(|(face, block)| (face.canonical_bytes(), *block));
    let mut articulation_memberships = Vec::new();
    let mut cursor = 0usize;
    while cursor < occurrences.len() {
        let face = occurrences[cursor].0;
        let mut end = cursor + 1;
        while end < occurrences.len() && occurrences[end].0 == face {
            end += 1;
        }
        if end - cursor > 1 {
            let mut memberships = Vec::new();
            for (_, block) in &occurrences[cursor..end] {
                if memberships.last() == Some(block) {
                    return false;
                }
                memberships.push(*block);
            }
            articulation_memberships.push(memberships);
        }
        cursor = end;
    }
    if articulation_memberships.is_empty() {
        return false;
    }
    let Some(node_count) = blocks.len().checked_add(articulation_memberships.len()) else {
        return false;
    };
    let Some(edge_count) = articulation_memberships
        .iter()
        .try_fold(0usize, |sum, memberships| {
            sum.checked_add(memberships.len())
        })
    else {
        return false;
    };
    if edge_count != node_count.saturating_sub(1) {
        return false;
    }
    let mut adjacency = vec![Vec::new(); node_count];
    for (articulation_index, memberships) in articulation_memberships.iter().enumerate() {
        let articulation_node = blocks.len() + articulation_index;
        for &block in memberships {
            if block >= blocks.len() {
                return false;
            }
            adjacency[block].push(articulation_node);
            adjacency[articulation_node].push(block);
        }
    }
    let mut visited = vec![false; node_count];
    let mut pending = vec![0usize];
    visited[0] = true;
    while let Some(node) = pending.pop() {
        for &neighbor in &adjacency[node] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                pending.push(neighbor);
            }
        }
    }
    visited.into_iter().all(|seen| seen)
}

fn block_articulation_incidence_is_tree_with_checkpoint_v1(
    blocks: &[CanonicalBlockBindingV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    if blocks.len() < 2 {
        return Ok(false);
    }
    let mut occurrences = Vec::new();
    for (block_index, block) in blocks.iter().enumerate() {
        checkpoint()?;
        for face in &block.faces {
            checkpoint()?;
            occurrences.push((*face, block_index));
        }
    }
    occurrences.sort_unstable_by_key(|(face, block)| (face.canonical_bytes(), *block));
    let mut articulation_memberships = Vec::new();
    let mut cursor = 0usize;
    while cursor < occurrences.len() {
        checkpoint()?;
        let face = occurrences[cursor].0;
        let mut end = cursor + 1;
        while end < occurrences.len() && occurrences[end].0 == face {
            checkpoint()?;
            end += 1;
        }
        if end - cursor > 1 {
            let mut memberships = Vec::new();
            for (_, block) in &occurrences[cursor..end] {
                checkpoint()?;
                if memberships.last() == Some(block) {
                    return Ok(false);
                }
                memberships.push(*block);
            }
            articulation_memberships.push(memberships);
        }
        cursor = end;
    }
    if articulation_memberships.is_empty() {
        return Ok(false);
    }
    let Some(node_count) = blocks.len().checked_add(articulation_memberships.len()) else {
        return Ok(false);
    };
    let Some(edge_count) = articulation_memberships
        .iter()
        .try_fold(0usize, |sum, memberships| {
            sum.checked_add(memberships.len())
        })
    else {
        return Ok(false);
    };
    if edge_count != node_count.saturating_sub(1) {
        return Ok(false);
    }
    let mut adjacency = vec![Vec::new(); node_count];
    for (articulation_index, memberships) in articulation_memberships.iter().enumerate() {
        checkpoint()?;
        let articulation_node = blocks.len() + articulation_index;
        for &block in memberships {
            checkpoint()?;
            if block >= blocks.len() {
                return Ok(false);
            }
            adjacency[block].push(articulation_node);
            adjacency[articulation_node].push(block);
        }
    }
    let mut visited = vec![false; node_count];
    let mut pending = vec![0usize];
    visited[0] = true;
    while let Some(node) = pending.pop() {
        checkpoint()?;
        for &neighbor in &adjacency[node] {
            checkpoint()?;
            if !visited[neighbor] {
                visited[neighbor] = true;
                pending.push(neighbor);
            }
        }
    }
    for seen in visited {
        checkpoint()?;
        if !seen {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Issues bounded tree-composition authority for exactly the supplied blocks.
///
/// No inference is made that the supplied hinge union exhausts any external
/// material graph. Callers must not use this result as project mutation
/// authority without an independent complete-live-graph union binding.
pub fn issue_multi_block_closure_authority_v1(
    inputs: Vec<MultiBlockClosureInputV1<'_>>,
    thickness: f64,
    issuer_context: [u8; 32],
) -> Option<MultiBlockClosureAuthorityV1> {
    issue_multi_block_closure_authority_with_scope_v1(
        inputs,
        thickness,
        issuer_context,
        MultiBlockAdmissionScopeV1::GenericSubmitted2To8,
    )
}

/// Issues a non-authorizing parent for exactly nine submitted blocks.
///
/// The private scope derives both its exact arity and distinct domain tag, so
/// this branch cannot be confused with the frozen generic 2..=8 authority.
pub fn issue_exact_nine_block_closure_authority_v1(
    inputs: Vec<MultiBlockClosureInputV1<'_>>,
    thickness: f64,
    issuer_context: [u8; 32],
) -> Option<MultiBlockClosureAuthorityV1> {
    issue_multi_block_closure_authority_with_scope_v1(
        inputs,
        thickness,
        issuer_context,
        MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
    )
}

/// Issues a non-authorizing parent for exactly ten submitted blocks.
///
/// The private scope derives both its exact arity and distinct domain tag, so
/// this branch cannot be confused with either frozen predecessor authority.
pub fn issue_exact_ten_block_closure_authority_v1(
    inputs: Vec<MultiBlockClosureInputV1<'_>>,
    thickness: f64,
    issuer_context: [u8; 32],
) -> Option<MultiBlockClosureAuthorityV1> {
    issue_multi_block_closure_authority_with_scope_v1(
        inputs,
        thickness,
        issuer_context,
        MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
    )
}

fn issue_multi_block_closure_authority_with_scope_v1(
    inputs: Vec<MultiBlockClosureInputV1<'_>>,
    thickness: f64,
    issuer_context: [u8; 32],
    scope: MultiBlockAdmissionScopeV1,
) -> Option<MultiBlockClosureAuthorityV1> {
    if !scope.admits_block_count_v1(inputs.len())
        || !thickness.is_finite()
        || thickness <= 0.0
        || issuer_context == [0; 32]
    {
        return None;
    }
    let mut observed_edges = HashSet::new();
    let mut blocks = Vec::with_capacity(inputs.len());
    for input in inputs {
        let fixed_face = input.closure.fixed_face();
        if !input
            .schedule
            .matches_binding(input.geometry, input.audit, fixed_face)
            || !input.closure.every_leaf_covers_graph_v1(input.geometry)
            || input.schedule.evaluate(0.0).is_none()
            || input.schedule.evaluate(1.0).is_none()
        {
            return None;
        }
        let mut edges = input
            .geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if edges.is_empty() || edges.iter().any(|edge| !observed_edges.insert(*edge)) {
            return None;
        }
        let mut faces = input.geometry.face_ids().to_vec();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if faces
            .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
            .is_err()
        {
            return None;
        }
        blocks.push(OwnedMultiBlockV1 {
            geometry: input.geometry.clone(),
            audit: input.audit.clone(),
            schedule: input.schedule.clone(),
            closure: input.closure.clone(),
            edges,
            faces,
        });
    }
    blocks.sort_unstable_by_key(|block| block.edges[0].canonical_bytes());
    let canonical = blocks
        .iter()
        .map(|block| CanonicalBlockBindingV1 {
            edges: block.edges.clone(),
            faces: block.faces.clone(),
        })
        .collect::<Vec<_>>();
    if !block_articulation_incidence_is_tree_v1(&canonical) {
        return None;
    }
    for (index, block) in blocks.iter().enumerate() {
        let fixed_face = block.closure.fixed_face();
        if !blocks
            .iter()
            .enumerate()
            .any(|(other_index, other)| other_index != index && other.faces.contains(&fixed_face))
        {
            return None;
        }
    }
    let mut hash = Sha256::new();
    hash.update(MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    hash.update(scope.closure_domain_tag_v1());
    hash.update(thickness.to_bits().to_le_bytes());
    hash.update(issuer_context);
    for block in &blocks {
        hash.update(block.schedule.graph_binding_fingerprint_v1());
        hash.update(block.schedule.certificate_binding_fingerprint_v2());
        hash.update(block.closure.partition_binding_fingerprint_v2());
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    Some(MultiBlockClosureAuthorityV1 {
        scope,
        binding: hash.finalize().into(),
        blocks,
        thickness_bits: thickness.to_bits(),
        issuer_context,
    })
}

pub fn issue_multi_block_positive_layer_authority_v1(
    parent: MultiBlockClosureAuthorityV1,
    mut inputs: Vec<MultiBlockPositiveLayerInputV1<'_>>,
    articulation_layer_fingerprint: [u8; 32],
) -> Option<MultiBlockPositiveLayerAuthorityV1> {
    let scope = parent.scope;
    if !scope.admits_block_count_v1(parent.blocks.len())
        || inputs.len() != parent.blocks.len()
        || articulation_layer_fingerprint == [0; 32]
    {
        return None;
    }
    inputs.sort_unstable_by_key(|input| {
        input
            .geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge().canonical_bytes())
            .min()
            .unwrap_or([0; 16])
    });
    let thickness = f64::from_bits(parent.thickness_bits);
    for (block, input) in parent.blocks.iter().zip(&inputs) {
        let fixed_face = block.closure.fixed_face();
        let mut edges = input
            .geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let mut faces = input.geometry.face_ids().to_vec();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if edges != block.edges
            || faces != block.faces
            || !input.positive.is_for(
                input.geometry,
                &block.audit,
                fixed_face,
                &block.schedule,
                &block.closure,
                thickness,
            )
            || !input.layer.is_for(
                input.geometry,
                input.source,
                &block.schedule,
                &block.closure,
                thickness,
            )
        {
            return None;
        }
    }
    let (positive, layer): (Vec<_>, Vec<_>) = inputs
        .into_iter()
        .map(|input| (input.positive, input.layer))
        .unzip();
    let binding = multi_block_positive_layer_binding_v1(
        scope,
        parent.binding,
        &layer,
        articulation_layer_fingerprint,
    );
    Some(MultiBlockPositiveLayerAuthorityV1 {
        scope,
        binding,
        parent,
        positive,
        layer,
        articulation_layer_fingerprint,
    })
}

/// Seals an already-issued submitted-set authority to one exact live geometry.
///
/// The consumed gap report is unforgeable outside this module and retains the
/// live geometry instance on which the canonical block union was observed.
/// The returned authority deliberately remains non-authorizing for motion,
/// Apply, project mutation, and viewer publication.
#[allow(clippy::too_many_arguments)]
pub fn issue_complete_multi_block_positive_layer_authority_v1(
    live_geometry: &MaterialHingeGraphGeometry,
    report: BlockUnionCompletenessGapReportV1,
    parent: MultiBlockPositiveLayerAuthorityV1,
    sources: &[&LayerOrderSnapshot],
    thickness: f64,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
    target_angles: &[(EdgeId, f64)],
) -> Option<CompleteMultiBlockPositiveLayerAuthorityV1> {
    if !complete_multi_block_report_matches_parent_v1(live_geometry, &report, &parent)
        || !parent.revalidates_v1(
            sources,
            thickness,
            issuer_context,
            articulation_layer_fingerprint,
        )
        || !parent.target_angles_match_v1(target_angles)
    {
        return None;
    }
    let scope = parent.scope;
    let binding = complete_multi_block_positive_layer_binding_v1(
        scope,
        parent.binding,
        &report.live_faces,
        &report.live_hinges,
        &report.blocks,
    );
    let authority = CompleteMultiBlockPositiveLayerAuthorityV1 {
        scope,
        binding,
        issuer: report.issuer,
        live_faces: report.live_faces,
        live_hinges: report.live_hinges,
        blocks: report.blocks,
        parent,
    };
    authority
        .revalidates_v1(
            live_geometry,
            sources,
            thickness,
            issuer_context,
            articulation_layer_fingerprint,
            target_angles,
        )
        .then_some(authority)
}

fn complete_multi_block_report_matches_parent_v1(
    live_geometry: &MaterialHingeGraphGeometry,
    report: &BlockUnionCompletenessGapReportV1,
    parent: &MultiBlockPositiveLayerAuthorityV1,
) -> bool {
    report.scope == parent.scope
        && parent.scope == parent.parent.scope
        && report.scope.admits_block_count_v1(report.blocks.len())
        && report.is_for(live_geometry)
        && report.complete
        && complete_block_union_matches_live_v1(
            report.scope,
            &report.blocks,
            &report.live_faces,
            &report.live_hinges,
        )
        && owned_multi_block_bindings_v1(parent) == report.blocks
}

#[cfg(test)]
pub(crate) fn complete_multi_block_report_matches_parent_for_test_v1(
    live_geometry: &MaterialHingeGraphGeometry,
    report: &BlockUnionCompletenessGapReportV1,
    parent: &MultiBlockPositiveLayerAuthorityV1,
) -> bool {
    complete_multi_block_report_matches_parent_v1(live_geometry, report, parent)
}

fn owned_multi_block_bindings_v1(
    authority: &MultiBlockPositiveLayerAuthorityV1,
) -> Vec<CanonicalBlockBindingV1> {
    authority
        .parent
        .blocks
        .iter()
        .map(|block| CanonicalBlockBindingV1 {
            edges: block.edges.clone(),
            faces: block.faces.clone(),
        })
        .collect()
}

fn owned_multi_block_bindings_with_checkpoint_v1(
    authority: &MultiBlockPositiveLayerAuthorityV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<Vec<CanonicalBlockBindingV1>, CommonArticulationContinuousLayerPathErrorV1> {
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(authority.parent.blocks.len())
        .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
    for block in &authority.parent.blocks {
        checkpoint()?;
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(block.edges.len())
            .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
        for edge in &block.edges {
            checkpoint()?;
            edges.push(*edge);
        }
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(block.faces.len())
            .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
        for face in &block.faces {
            checkpoint()?;
            faces.push(*face);
        }
        bindings.push(CanonicalBlockBindingV1 { edges, faces });
    }
    Ok(bindings)
}

fn complete_block_union_matches_live_v1(
    scope: MultiBlockAdmissionScopeV1,
    blocks: &[CanonicalBlockBindingV1],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
) -> bool {
    if !scope.admits_block_count_v1(blocks.len())
        || blocks.iter().any(|block| {
            block.faces.is_empty()
                || block.edges.is_empty()
                || block
                    .faces
                    .windows(2)
                    .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
                || block
                    .edges
                    .windows(2)
                    .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
        })
        || !block_articulation_incidence_is_tree_v1(blocks)
    {
        return false;
    }
    let mut face_union = blocks
        .iter()
        .flat_map(|block| block.faces.iter().copied())
        .collect::<Vec<_>>();
    face_union.sort_unstable_by_key(FaceId::canonical_bytes);
    face_union.dedup();
    let mut hinge_union = blocks
        .iter()
        .flat_map(|block| block.edges.iter().copied())
        .collect::<Vec<_>>();
    hinge_union.sort_unstable_by_key(EdgeId::canonical_bytes);
    !hinge_union.windows(2).any(|pair| pair[0] == pair[1])
        && face_union == live_faces
        && hinge_union == live_hinges
}

fn complete_block_union_matches_live_with_checkpoint_v1(
    scope: MultiBlockAdmissionScopeV1,
    blocks: &[CanonicalBlockBindingV1],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    if !scope.admits_block_count_v1(blocks.len()) {
        return Ok(false);
    }
    for block in blocks {
        checkpoint()?;
        if block.faces.is_empty() || block.edges.is_empty() {
            return Ok(false);
        }
        for pair in block.faces.windows(2) {
            checkpoint()?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Ok(false);
            }
        }
        for pair in block.edges.windows(2) {
            checkpoint()?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Ok(false);
            }
        }
    }
    if !block_articulation_incidence_is_tree_with_checkpoint_v1(blocks, checkpoint)? {
        return Ok(false);
    }
    let mut face_union = Vec::new();
    let mut hinge_union = Vec::new();
    for block in blocks {
        checkpoint()?;
        for face in &block.faces {
            checkpoint()?;
            face_union.push(*face);
        }
        for edge in &block.edges {
            checkpoint()?;
            hinge_union.push(*edge);
        }
    }
    face_union.sort_unstable_by_key(FaceId::canonical_bytes);
    face_union.dedup();
    hinge_union.sort_unstable_by_key(EdgeId::canonical_bytes);
    for pair in hinge_union.windows(2) {
        checkpoint()?;
        if pair[0] == pair[1] {
            return Ok(false);
        }
    }
    Ok(face_union == live_faces && hinge_union == live_hinges)
}

fn complete_multi_block_positive_layer_binding_v1(
    scope: MultiBlockAdmissionScopeV1,
    parent_binding: [u8; 32],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
    blocks: &[CanonicalBlockBindingV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    if let Some(domain_tag) = scope.complete_live_domain_tag_v1() {
        hash.update(domain_tag);
    }
    hash.update(parent_binding);
    hash.update((live_faces.len() as u64).to_le_bytes());
    for face in live_faces {
        hash.update(face.canonical_bytes());
    }
    hash.update((live_hinges.len() as u64).to_le_bytes());
    for hinge in live_hinges {
        hash.update(hinge.canonical_bytes());
    }
    hash.update((blocks.len() as u64).to_le_bytes());
    for block in blocks {
        hash.update((block.faces.len() as u64).to_le_bytes());
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
    }
    hash.finalize().into()
}

fn complete_multi_block_positive_layer_binding_with_checkpoint_v1(
    scope: MultiBlockAdmissionScopeV1,
    parent_binding: [u8; 32],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
    blocks: &[CanonicalBlockBindingV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<[u8; 32], CommonArticulationContinuousLayerPathErrorV1> {
    let mut hash = Sha256::new();
    hash.update(COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    if let Some(domain_tag) = scope.complete_live_domain_tag_v1() {
        hash.update(domain_tag);
    }
    hash.update(parent_binding);
    hash.update((live_faces.len() as u64).to_le_bytes());
    for face in live_faces {
        checkpoint()?;
        hash.update(face.canonical_bytes());
    }
    hash.update((live_hinges.len() as u64).to_le_bytes());
    for hinge in live_hinges {
        checkpoint()?;
        hash.update(hinge.canonical_bytes());
    }
    hash.update((blocks.len() as u64).to_le_bytes());
    for block in blocks {
        checkpoint()?;
        hash.update((block.faces.len() as u64).to_le_bytes());
        for face in &block.faces {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
    }
    checkpoint()?;
    Ok(hash.finalize().into())
}

fn multi_block_positive_layer_binding_v1(
    scope: MultiBlockAdmissionScopeV1,
    parent_binding: [u8; 32],
    layers: &[GeneralMultiFaceCellTransportProofV1],
    articulation_layer_fingerprint: [u8; 32],
) -> [u8; 32] {
    let mut records = layers
        .iter()
        .map(|proof| {
            (
                proof.target_order_hash(),
                proof.paper_thickness_mm().to_bits(),
                proof.transition_hashes().len(),
                proof.pair_order_count(),
            )
        })
        .collect::<Vec<_>>();
    records.sort_unstable();
    let mut hash = Sha256::new();
    hash.update(MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    if let Some(domain_tag) = scope.positive_layer_domain_tag_v1() {
        hash.update(domain_tag);
    }
    hash.update(parent_binding);
    hash.update(articulation_layer_fingerprint);
    for (target, thickness, transitions, pairs) in records {
        hash.update(target);
        hash.update(thickness.to_le_bytes());
        hash.update((transitions as u64).to_le_bytes());
        hash.update((pairs as u64).to_le_bytes());
    }
    hash.finalize().into()
}

fn multi_block_positive_layer_binding_with_checkpoint_v1(
    scope: MultiBlockAdmissionScopeV1,
    parent_binding: [u8; 32],
    layers: &[GeneralMultiFaceCellTransportProofV1],
    articulation_layer_fingerprint: [u8; 32],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<[u8; 32], CommonArticulationContinuousLayerPathErrorV1> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(layers.len())
        .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
    for proof in layers {
        checkpoint()?;
        records.push((
            proof.target_order_hash(),
            proof.paper_thickness_mm().to_bits(),
            proof.transition_hashes().len(),
            proof.pair_order_count(),
        ));
    }
    records.sort_unstable();
    let mut hash = Sha256::new();
    hash.update(MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    if let Some(domain_tag) = scope.positive_layer_domain_tag_v1() {
        hash.update(domain_tag);
    }
    hash.update(parent_binding);
    hash.update(articulation_layer_fingerprint);
    for (target, thickness, transitions, pairs) in records {
        checkpoint()?;
        hash.update(target);
        hash.update(thickness.to_le_bytes());
        hash.update((transitions as u64).to_le_bytes());
        hash.update((pairs as u64).to_le_bytes());
    }
    checkpoint()?;
    Ok(hash.finalize().into())
}

/// Research wrapper that binds already-issued whole-graph parent proofs to one
/// canonical edge partition.
///
/// This wrapper does not independently prove clearance or layer transport and
/// does not, by itself, authorize continuous motion or project mutation. The
/// caller remains responsible for supplying a canonical partition and trusted
/// articulation pose/layer fingerprints from an authority appropriate to its
/// application. Callers can neither manufacture a partial block proof nor
/// substitute a pose/layer snapshot after issuance.
pub struct BlockComposedPathAuthorityV1 {
    binding: [u8; 32],
    blocks: Vec<CanonicalBlockBindingV1>,
    positive: PositiveThicknessContinuousCertificateV1,
    layer: GeneralMultiFaceCellTransportProofV1,
}

impl BlockComposedPathAuthorityV1 {
    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn revalidates_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        source: &LayerOrderSnapshot,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        thickness: f64,
        articulation_pose_fingerprint: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
    ) -> bool {
        self.positive
            .is_for(geometry, audit, fixed_face, schedule, closure, thickness)
            && self
                .layer
                .is_for(geometry, source, schedule, closure, thickness)
            && self.binding
                == block_binding_v1(
                    schedule,
                    closure,
                    &self.blocks,
                    articulation_pose_fingerprint,
                    articulation_layer_fingerprint,
                )
    }

    pub fn into_parent_proofs(
        self,
    ) -> (
        PositiveThicknessContinuousCertificateV1,
        GeneralMultiFaceCellTransportProofV1,
    ) {
        (self.positive, self.layer)
    }
}

fn block_binding_v1(
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    blocks: &[CanonicalBlockBindingV1],
    articulation_pose_fingerprint: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(BLOCK_COMPOSED_PATH_MODEL_ID_V1.as_bytes());
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(closure.partition_binding_fingerprint_v2());
    hash.update(articulation_pose_fingerprint);
    hash.update(articulation_layer_fingerprint);
    for block in blocks {
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    hash.finalize().into()
}

#[allow(clippy::too_many_arguments)]
/// Binds existing whole-graph parent proofs to a caller-supplied block
/// partition for research and composition workflows.
///
/// Issuance authenticates the parent proofs and validates the submitted edge
/// partition, but it does not establish the provenance of the two articulation
/// fingerprints. Supplying trusted articulation fingerprints and choosing the
/// canonical application partition remain caller responsibilities. The
/// returned wrapper alone grants neither continuous-motion nor
/// project-mutation authority.
pub fn issue_block_composed_path_authority_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    source: &LayerOrderSnapshot,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    thickness: f64,
    positive: PositiveThicknessContinuousCertificateV1,
    layer: GeneralMultiFaceCellTransportProofV1,
    blocks: Vec<Vec<EdgeId>>,
    articulation_pose_fingerprint: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
) -> Option<BlockComposedPathAuthorityV1> {
    if blocks.len() < 2
        || blocks.len() > BLOCK_COMPOSITION_LIMIT_V1
        || articulation_pose_fingerprint == [0; 32]
        || articulation_layer_fingerprint == [0; 32]
        || !positive.is_for(geometry, audit, fixed_face, schedule, closure, thickness)
        || !layer.is_for(geometry, source, schedule, closure, thickness)
    {
        return None;
    }
    let all_edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let mut observed = HashSet::new();
    let mut canonical = Vec::with_capacity(blocks.len());
    for mut edges in blocks {
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if edges.is_empty()
            || edges.windows(2).any(|pair| pair[0] == pair[1])
            || edges
                .iter()
                .any(|edge| !all_edges.contains(edge) || !observed.insert(*edge))
        {
            return None;
        }
        let mut face_set = HashSet::new();
        for edge in &edges {
            let hinge = geometry
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == *edge)?;
            face_set.insert(hinge.left_face());
            face_set.insert(hinge.right_face());
        }
        let mut faces = face_set.into_iter().collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        canonical.push(CanonicalBlockBindingV1 { edges, faces });
    }
    if observed.len() != all_edges.len() {
        return None;
    }
    canonical.sort_unstable_by_key(|block| block.edges[0].canonical_bytes());
    if !block_articulation_incidence_is_tree_v1(&canonical) {
        return None;
    }
    let binding = block_binding_v1(
        schedule,
        closure,
        &canonical,
        articulation_pose_fingerprint,
        articulation_layer_fingerprint,
    );
    Some(BlockComposedPathAuthorityV1 {
        binding,
        blocks: canonical,
        positive,
        layer,
    })
}

/// Exact live inputs for the staged common-articulation composition boundary.
///
/// `common_pose` and `clearance` are moved into a successful authority. This
/// boundary intentionally does not accept snapshots or caller-manufactured
/// fingerprints in place of either opaque prerequisite.
pub struct CommonArticulationBlockComposedPathInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ori_kinematics::ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose: CommonArticulationPoseAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance: CommonArticulationClearancePrerequisiteV1,
    pub clearance_limits: CommonArticulationClearanceLimitsV1,
    /// The existing caller-facing edge partition. It must exactly equal the
    /// canonical block decomposition, including every block face binding.
    pub blocks: Vec<Vec<EdgeId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationBlockComposedPathErrorV1 {
    #[error("the staged block-composition input is malformed")]
    InvalidInput,
    #[error("the staged block-composition operation exceeded a resource limit")]
    ResourceLimit,
    #[error("the submitted edge partition differs from the canonical decomposition")]
    CanonicalBlockPartitionMismatch,
    #[error("the common-articulation pose prerequisite failed exact revalidation: {0}")]
    CommonPose(CommonArticulationPoseErrorV1),
    #[error("the common-articulation clearance prerequisite failed exact revalidation: {0}")]
    Clearance(CommonArticulationClearanceErrorV1),
    #[error("the staged block-composition operation was cancelled")]
    Cancelled,
    #[error("the staged block-composition operation deadline elapsed")]
    DeadlineExceeded,
}

/// Opaque staged integration authority for P2-2.
///
/// This authority retains both non-cloneable prerequisites by move and binds
/// their fingerprints to the exact canonical decomposition. P2-3 is not
/// complete, so it grants no continuous-motion, collision-clearance, project,
/// Apply, or viewer authority.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationBlockComposedPathAuthorityV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationBlockComposedPathAuthorityV1>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationBlockComposedPathAuthorityV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationBlockComposedPathAuthorityV1>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationBlockComposedPathAuthorityV1 {
    binding: [u8; 32],
    blocks: Vec<CanonicalBlockBindingV1>,
    common_pose: CommonArticulationPoseAuthorityV1,
    clearance: CommonArticulationClearancePrerequisiteV1,
}

impl CommonArticulationBlockComposedPathAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_MODEL_ID_V1
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub const fn common_pose_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.common_pose.binding_fingerprint_v1()
    }

    #[must_use]
    pub const fn clearance_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.clearance.binding_fingerprint_v1()
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.blocks.len()
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

    #[allow(clippy::too_many_arguments)]
    fn revalidate_exact_with_control_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
        decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
        common_pose_limits: CommonArticulationPoseLimitsV1,
        schedule: &CanonicalCycleScheduleV1,
        schedule_limits: CycleScheduleLimitsV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        paper_thickness_mm: f64,
        clearance_limits: CommonArticulationClearanceLimitsV1,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CommonArticulationBlockComposedPathErrorV1> {
        staged_block_composition_checkpoint_v1(control)?;
        if !decomposition.is_for_geometry(geometry)
            || !pose.is_for_geometry(geometry)
            || !paper_thickness_mm.is_finite()
            || paper_thickness_mm <= 0.0
        {
            return Err(CommonArticulationBlockComposedPathErrorV1::InvalidInput);
        }
        self.common_pose
            .revalidate_with_checkpoint_v1(
                CommonArticulationPoseInputV1 {
                    geometry,
                    pose,
                    decomposition,
                    paper_thickness_mm,
                    limits: common_pose_limits,
                },
                || staged_common_pose_checkpoint_v1(control),
            )
            .map_err(map_staged_common_pose_error_v1)?;
        staged_block_composition_checkpoint_v1(control)?;
        self.clearance
            .revalidate_with_control_v1(
                CommonArticulationClearanceRevalidationInputV1 {
                    geometry,
                    audit,
                    pose,
                    decomposition,
                    common_pose: &self.common_pose,
                    common_pose_limits,
                    schedule,
                    schedule_limits,
                    closure,
                    paper_thickness_mm,
                    limits: clearance_limits,
                },
                control,
            )
            .map_err(map_staged_clearance_error_v1)?;
        staged_block_composition_checkpoint_v1(control)?;
        let blocks = canonical_decomposition_block_bindings_v1(decomposition, control)?;
        if blocks != self.blocks
            || self.binding
                != common_articulation_block_composed_binding_v1(
                    schedule,
                    closure,
                    paper_thickness_mm,
                    &blocks,
                    self.common_pose.binding_fingerprint_v1(),
                    self.clearance.binding_fingerprint_v1(),
                )
        {
            return Err(
                CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
            );
        }
        Ok(())
    }
}

pub fn issue_common_articulation_block_composed_path_authority_v1(
    input: CommonArticulationBlockComposedPathInputV1<'_>,
) -> Result<
    CommonArticulationBlockComposedPathAuthorityV1,
    CommonArticulationBlockComposedPathErrorV1,
> {
    issue_common_articulation_block_composed_path_authority_with_control_v1(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_block_composed_path_authority_with_control_v1(
    input: CommonArticulationBlockComposedPathInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    CommonArticulationBlockComposedPathAuthorityV1,
    CommonArticulationBlockComposedPathErrorV1,
> {
    staged_block_composition_checkpoint_v1(control)?;
    if !input.decomposition.is_for_geometry(input.geometry)
        || !input.pose.is_for_geometry(input.geometry)
        || !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
    {
        return Err(CommonArticulationBlockComposedPathErrorV1::InvalidInput);
    }

    staged_block_composition_checkpoint_v1(control)?;
    input
        .common_pose
        .revalidate_with_checkpoint_v1(
            CommonArticulationPoseInputV1 {
                geometry: input.geometry,
                pose: input.pose,
                decomposition: input.decomposition,
                paper_thickness_mm: input.paper_thickness_mm,
                limits: input.common_pose_limits,
            },
            || staged_common_pose_checkpoint_v1(control),
        )
        .map_err(map_staged_common_pose_error_v1)?;
    staged_block_composition_checkpoint_v1(control)?;
    input
        .clearance
        .revalidate_with_control_v1(
            CommonArticulationClearanceRevalidationInputV1 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                decomposition: input.decomposition,
                common_pose: &input.common_pose,
                common_pose_limits: input.common_pose_limits,
                schedule: input.schedule,
                schedule_limits: input.schedule_limits,
                closure: input.closure,
                paper_thickness_mm: input.paper_thickness_mm,
                limits: input.clearance_limits,
            },
            control,
        )
        .map_err(map_staged_clearance_error_v1)?;

    staged_block_composition_checkpoint_v1(control)?;
    let canonical = canonical_block_partition_for_staged_v1(input.geometry, input.blocks, control)?;
    let decomposition = canonical_decomposition_block_bindings_v1(input.decomposition, control)?;
    if canonical != decomposition {
        return Err(CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch);
    }

    staged_block_composition_checkpoint_v1(control)?;
    let binding = common_articulation_block_composed_binding_v1(
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        &canonical,
        input.common_pose.binding_fingerprint_v1(),
        input.clearance.binding_fingerprint_v1(),
    );
    staged_block_composition_checkpoint_v1(control)?;
    Ok(CommonArticulationBlockComposedPathAuthorityV1 {
        binding,
        blocks: canonical,
        common_pose: input.common_pose,
        clearance: input.clearance,
    })
}

fn canonical_block_partition_for_staged_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: Vec<Vec<EdgeId>>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Vec<CanonicalBlockBindingV1>, CommonArticulationBlockComposedPathErrorV1> {
    if blocks.len() < 2 || blocks.len() > BLOCK_COMPOSITION_LIMIT_V1 {
        return Err(CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    let mut all_edges = HashSet::new();
    all_edges
        .try_reserve(geometry.hinges().len())
        .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
    for hinge in geometry.hinges() {
        staged_block_composition_checkpoint_v1(control)?;
        if !all_edges.insert(hinge.edge()) {
            return Err(
                CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
            );
        }
    }
    let mut observed = HashSet::new();
    observed
        .try_reserve(all_edges.len())
        .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(blocks.len())
        .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
    for mut edges in blocks {
        staged_block_composition_checkpoint_v1(control)?;
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if edges.is_empty()
            || edges.windows(2).any(|pair| pair[0] == pair[1])
            || edges
                .iter()
                .any(|edge| !all_edges.contains(edge) || !observed.insert(*edge))
        {
            return Err(
                CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
            );
        }
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(
                edges
                    .len()
                    .checked_mul(2)
                    .ok_or(CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?,
            )
            .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
        for edge in &edges {
            staged_block_composition_checkpoint_v1(control)?;
            let hinge = geometry
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == *edge)
                .ok_or(
                    CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
                )?;
            faces.push(hinge.left_face());
            faces.push(hinge.right_face());
        }
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        faces.dedup();
        canonical.push(CanonicalBlockBindingV1 { edges, faces });
    }
    if observed.len() != all_edges.len() {
        return Err(CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    canonical.sort_unstable_by_key(|block| block.edges[0].canonical_bytes());
    if !block_articulation_incidence_is_tree_v1(&canonical) {
        return Err(CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    Ok(canonical)
}

fn canonical_decomposition_block_bindings_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Vec<CanonicalBlockBindingV1>, CommonArticulationBlockComposedPathErrorV1> {
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(decomposition.blocks().len())
        .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
    for block in decomposition.blocks() {
        staged_block_composition_checkpoint_v1(control)?;
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(block.geometry().hinges().len())
            .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
        for hinge in block.geometry().hinges() {
            staged_block_composition_checkpoint_v1(control)?;
            edges.push(hinge.edge());
        }
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        if edges.is_empty() || edges.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(
                CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
            );
        }
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(block.geometry().face_ids().len())
            .map_err(|_| CommonArticulationBlockComposedPathErrorV1::ResourceLimit)?;
        for face in block.geometry().face_ids() {
            staged_block_composition_checkpoint_v1(control)?;
            faces.push(*face);
        }
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if faces.is_empty() || faces.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(
                CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch,
            );
        }
        canonical.push(CanonicalBlockBindingV1 { edges, faces });
    }
    canonical.sort_unstable_by_key(|block| block.edges[0].canonical_bytes());
    if !block_articulation_incidence_is_tree_v1(&canonical) {
        return Err(CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    Ok(canonical)
}

fn common_articulation_block_composed_binding_v1(
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    blocks: &[CanonicalBlockBindingV1],
    common_pose_binding: [u8; 32],
    clearance_binding: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_MODEL_ID_V1.as_bytes());
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(closure.partition_binding_fingerprint_v2());
    hash.update(paper_thickness_mm.to_bits().to_be_bytes());
    hash.update(common_pose_binding);
    hash.update(clearance_binding);
    hash.update((blocks.len() as u64).to_be_bytes());
    for block in blocks {
        hash.update((block.edges.len() as u64).to_be_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_be_bytes());
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    hash.finalize().into()
}

fn map_staged_clearance_error_v1(
    error: CommonArticulationClearanceErrorV1,
) -> CommonArticulationBlockComposedPathErrorV1 {
    match error {
        CommonArticulationClearanceErrorV1::Cancelled => {
            CommonArticulationBlockComposedPathErrorV1::Cancelled
        }
        CommonArticulationClearanceErrorV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded
        }
        error => CommonArticulationBlockComposedPathErrorV1::Clearance(error),
    }
}

fn map_staged_common_pose_error_v1(
    error: CommonArticulationPoseErrorV1,
) -> CommonArticulationBlockComposedPathErrorV1 {
    match error {
        CommonArticulationPoseErrorV1::Cancelled => {
            CommonArticulationBlockComposedPathErrorV1::Cancelled
        }
        CommonArticulationPoseErrorV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded
        }
        error => CommonArticulationBlockComposedPathErrorV1::CommonPose(error),
    }
}

fn staged_common_pose_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationPoseStopV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => CommonArticulationPoseStopV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationPoseStopV1::DeadlineExceeded
        }
    })
}

fn staged_block_composition_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationBlockComposedPathErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationBlockComposedPathErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded
        }
    })
}

/// Exact live inputs for the final common-articulation continuous-layer
/// composition boundary.
///
/// All three opaque prerequisites are consumed on success. `whole_parent_layer`
/// must cover the complete parent geometry; per-block layer proofs alone are
/// insufficient at this boundary.
pub struct CommonArticulationContinuousLayerPathInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ori_kinematics::ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub staged: CommonArticulationBlockComposedPathAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance_limits: CommonArticulationClearanceLimitsV1,
    pub complete: CompleteMultiBlockPositiveLayerAuthorityV1,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub source: &'a LayerOrderSnapshot,
    pub whole_parent_layer: GeneralMultiFaceCellTransportProofV1,
}

/// Exact live inputs required to revalidate a retained final authority.
///
/// The three nested authorities are deliberately absent: revalidation uses
/// the opaque prerequisites retained inside the final authority itself.
#[derive(Clone, Copy)]
pub struct CommonArticulationContinuousLayerPathRevalidationInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ori_kinematics::ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose_limits: CommonArticulationPoseLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance_limits: CommonArticulationClearanceLimitsV1,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub source: &'a LayerOrderSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationContinuousLayerPathErrorV1 {
    #[error("the final common-articulation path input is malformed")]
    InvalidInput,
    #[error("the staged common-articulation authority failed exact revalidation: {0}")]
    Staged(CommonArticulationBlockComposedPathErrorV1),
    #[error("the complete multi-block positive-layer authority failed exact revalidation")]
    CompleteMultiBlockMismatch,
    #[error("the staged and complete authorities bind different canonical blocks")]
    CanonicalBlockPartitionMismatch,
    #[error("a complete-authority block schedule is not the exact full-path restriction")]
    BlockScheduleRestrictionMismatch,
    #[error("a block layer source is not the exact whole-parent source restriction")]
    BlockSourceRestrictionMismatch,
    #[error("the whole-parent layer transport proof failed exact revalidation")]
    WholeParentLayerMismatch,
    #[error("the retained final authority binding does not match the live inputs")]
    BindingMismatch,
    #[error("the final common-articulation path operation exceeded a resource limit")]
    ResourceLimit,
    #[error("the final common-articulation path operation was cancelled")]
    Cancelled,
    #[error("the final common-articulation path operation deadline elapsed")]
    DeadlineExceeded,
}

/// Opaque P2-3 authority combining exact common articulation, continuous
/// clearance, complete blockwise positive-layer coverage, and whole-parent
/// layer transport.
///
/// This is path evidence only. Project mutation, Apply, and viewer publication
/// remain separate permission boundaries.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationContinuousLayerPathAuthorityV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationContinuousLayerPathAuthorityV1>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationContinuousLayerPathAuthorityV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationContinuousLayerPathAuthorityV1>();
/// ```
pub struct CommonArticulationContinuousLayerPathAuthorityV1 {
    binding: [u8; 32],
    staged: CommonArticulationBlockComposedPathAuthorityV1,
    complete: CompleteMultiBlockPositiveLayerAuthorityV1,
    whole_parent_layer: GeneralMultiFaceCellTransportProofV1,
}

impl std::fmt::Debug for CommonArticulationContinuousLayerPathAuthorityV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommonArticulationContinuousLayerPathAuthorityV1")
            .field("binding", &self.binding)
            .field("block_count", &self.staged.blocks.len())
            .finish_non_exhaustive()
    }
}

impl CommonArticulationContinuousLayerPathAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.staged.blocks.len()
    }

    #[must_use]
    pub const fn staged_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.staged.binding_fingerprint_v1()
    }

    #[must_use]
    pub const fn complete_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.complete.binding_fingerprint_v1()
    }

    #[cfg(test)]
    pub(crate) fn corrupt_complete_scope_for_test_v1(&mut self) {
        self.complete.corrupt_scope_for_test_v1();
    }

    #[cfg(test)]
    pub(crate) fn corrupt_complete_partition_for_test_v1(&mut self) {
        self.complete.corrupt_partition_for_test_v1();
    }

    #[must_use]
    pub fn whole_parent_target_order_hash_v1(&self) -> [u8; 32] {
        self.whole_parent_layer.target_order_hash()
    }

    pub fn revalidate_v1(
        &self,
        input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
    ) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
        self.revalidate_with_control_v1(input, &CooperativeOperationControlV1::unbounded())
    }

    pub fn revalidate_with_control_v1(
        &self,
        input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
        let mut checkpoint = || final_path_checkpoint_v1(control);
        self.revalidate_with_checkpoint_v1(input, control, &mut checkpoint)
    }

    fn revalidate_with_checkpoint_v1(
        &self,
        input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
        control: &CooperativeOperationControlV1<'_>,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
    ) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
        let binding = validate_common_articulation_continuous_layer_path_v1(
            input,
            &self.staged,
            &self.complete,
            &self.whole_parent_layer,
            control,
            checkpoint,
        )?;
        checkpoint()?;
        if binding != self.binding {
            return Err(CommonArticulationContinuousLayerPathErrorV1::BindingMismatch);
        }
        checkpoint()?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn revalidate_with_checkpoint_for_test_v1(
        &self,
        input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
    ) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
        self.revalidate_with_checkpoint_v1(
            input,
            &CooperativeOperationControlV1::unbounded(),
            &mut checkpoint,
        )
    }

    #[cfg(test)]
    pub(crate) fn corrupt_binding_for_test_v1(&mut self) {
        self.binding[0] ^= 1;
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        true
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

pub fn issue_common_articulation_continuous_layer_path_authority_v1(
    input: CommonArticulationContinuousLayerPathInputV1<'_>,
) -> Result<
    CommonArticulationContinuousLayerPathAuthorityV1,
    CommonArticulationContinuousLayerPathErrorV1,
> {
    issue_common_articulation_continuous_layer_path_authority_with_control_v1(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_continuous_layer_path_authority_with_control_v1(
    input: CommonArticulationContinuousLayerPathInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    CommonArticulationContinuousLayerPathAuthorityV1,
    CommonArticulationContinuousLayerPathErrorV1,
> {
    let live_input = CommonArticulationContinuousLayerPathRevalidationInputV1 {
        geometry: input.geometry,
        audit: input.audit,
        pose: input.pose,
        decomposition: input.decomposition,
        common_pose_limits: input.common_pose_limits,
        schedule: input.schedule,
        schedule_limits: input.schedule_limits,
        closure: input.closure,
        paper_thickness_mm: input.paper_thickness_mm,
        clearance_limits: input.clearance_limits,
        block_sources: input.block_sources,
        issuer_context: input.issuer_context,
        articulation_layer_fingerprint: input.articulation_layer_fingerprint,
        target_angles: input.target_angles,
        source: input.source,
    };
    let mut checkpoint = || final_path_checkpoint_v1(control);
    let binding = validate_common_articulation_continuous_layer_path_v1(
        live_input,
        &input.staged,
        &input.complete,
        &input.whole_parent_layer,
        control,
        &mut checkpoint,
    )?;
    checkpoint()?;
    Ok(CommonArticulationContinuousLayerPathAuthorityV1 {
        binding,
        staged: input.staged,
        complete: input.complete,
        whole_parent_layer: input.whole_parent_layer,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_common_articulation_continuous_layer_path_v1(
    input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
    staged: &CommonArticulationBlockComposedPathAuthorityV1,
    complete: &CompleteMultiBlockPositiveLayerAuthorityV1,
    whole_parent_layer: &GeneralMultiFaceCellTransportProofV1,
    control: &CooperativeOperationControlV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<[u8; 32], CommonArticulationContinuousLayerPathErrorV1> {
    checkpoint()?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || input.issuer_context == [0; 32]
        || input.articulation_layer_fingerprint == [0; 32]
    {
        return Err(CommonArticulationContinuousLayerPathErrorV1::InvalidInput);
    }
    staged
        .revalidate_exact_with_control_v1(
            input.geometry,
            input.audit,
            input.pose,
            input.decomposition,
            input.common_pose_limits,
            input.schedule,
            input.schedule_limits,
            input.closure,
            input.paper_thickness_mm,
            input.clearance_limits,
            control,
        )
        .map_err(map_final_staged_error_v1)?;
    checkpoint()?;
    if !complete.revalidates_with_checkpoint_v1(
        input.geometry,
        input.block_sources,
        input.paper_thickness_mm,
        input.issuer_context,
        input.articulation_layer_fingerprint,
        input.target_angles,
        checkpoint,
    )? {
        return Err(CommonArticulationContinuousLayerPathErrorV1::CompleteMultiBlockMismatch);
    }
    if !canonical_block_bindings_equal_with_checkpoint_v1(
        &staged.blocks,
        &complete.blocks,
        checkpoint,
    )? {
        return Err(CommonArticulationContinuousLayerPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    checkpoint()?;
    if !layer_proof_is_for_with_final_checkpoint_v1(
        whole_parent_layer,
        input.geometry,
        input.source,
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        checkpoint,
    )? {
        return Err(CommonArticulationContinuousLayerPathErrorV1::WholeParentLayerMismatch);
    }
    validate_complete_block_schedule_restrictions_v1(
        input.geometry,
        input.audit,
        input.schedule,
        complete,
        checkpoint,
    )?;
    validate_block_source_restrictions_v1(
        input.source,
        &complete.blocks,
        input.block_sources,
        checkpoint,
    )?;
    let canonical_target_angles =
        canonical_target_angles_for_final_path_v1(input.target_angles, checkpoint)?;
    common_articulation_continuous_layer_path_binding_with_checkpoint_v1(
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        &staged.blocks,
        staged.binding,
        complete.binding,
        whole_parent_layer,
        input.issuer_context,
        input.articulation_layer_fingerprint,
        &canonical_target_angles,
        checkpoint,
    )
}

#[allow(clippy::too_many_arguments)]
fn common_articulation_continuous_layer_path_binding_with_checkpoint_v1(
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    blocks: &[CanonicalBlockBindingV1],
    staged_binding: [u8; 32],
    complete_binding: [u8; 32],
    whole_parent_layer: &GeneralMultiFaceCellTransportProofV1,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
    canonical_target_angles: &[(EdgeId, f64)],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<[u8; 32], CommonArticulationContinuousLayerPathErrorV1> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1.as_bytes());
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(closure.partition_binding_fingerprint_v2());
    hash.update(paper_thickness_mm.to_bits().to_be_bytes());
    hash.update(staged_binding);
    hash.update(complete_binding);
    hash.update((whole_parent_layer.transition_hashes().len() as u64).to_be_bytes());
    for transition_hash in whole_parent_layer.transition_hashes() {
        checkpoint()?;
        hash.update(transition_hash);
    }
    hash.update((whole_parent_layer.pair_order_count() as u64).to_be_bytes());
    hash.update(issuer_context);
    hash.update(articulation_layer_fingerprint);
    hash.update((blocks.len() as u64).to_be_bytes());
    for block in blocks {
        checkpoint()?;
        hash.update((block.edges.len() as u64).to_be_bytes());
        for edge in &block.edges {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_be_bytes());
        for face in &block.faces {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
    }
    hash.update((canonical_target_angles.len() as u64).to_be_bytes());
    for (edge, angle) in canonical_target_angles {
        checkpoint()?;
        hash.update(edge.canonical_bytes());
        hash.update(angle.to_bits().to_be_bytes());
    }
    checkpoint()?;
    Ok(hash.finalize().into())
}

fn canonical_target_angles_for_final_path_v1(
    target_angles: &[(EdgeId, f64)],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<Vec<(EdgeId, f64)>, CommonArticulationContinuousLayerPathErrorV1> {
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(target_angles.len())
        .map_err(|_| CommonArticulationContinuousLayerPathErrorV1::ResourceLimit)?;
    for angle in target_angles {
        checkpoint()?;
        canonical.push(*angle);
    }
    canonical.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    checkpoint()?;
    Ok(canonical)
}

fn canonical_block_bindings_equal_with_checkpoint_v1(
    expected: &[CanonicalBlockBindingV1],
    actual: &[CanonicalBlockBindingV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        checkpoint()?;
        if !slice_equal_with_final_checkpoint_v1(&expected.edges, &actual.edges, checkpoint)?
            || !slice_equal_with_final_checkpoint_v1(&expected.faces, &actual.faces, checkpoint)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn restrict_schedule_with_final_checkpoint_v1(
    schedule: &CanonicalCycleScheduleV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    block_geometry: &MaterialHingeGraphGeometry,
    block_audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<CanonicalCycleScheduleV1, CommonArticulationContinuousLayerPathErrorV1> {
    let mut unexpected_checkpoint_error = None;
    let result = schedule.restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
        geometry,
        audit,
        block_geometry,
        block_audit,
        fixed_face,
        || match checkpoint() {
            Ok(()) => Ok(()),
            Err(CommonArticulationContinuousLayerPathErrorV1::Cancelled) => {
                Err(CycleScheduleRestrictionStopV1::Cancelled)
            }
            Err(CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded) => {
                Err(CycleScheduleRestrictionStopV1::DeadlineExceeded)
            }
            Err(error) => {
                unexpected_checkpoint_error = Some(error);
                Err(CycleScheduleRestrictionStopV1::Cancelled)
            }
        },
    );
    if let Some(error) = unexpected_checkpoint_error {
        return Err(error);
    }
    result.map_err(|error| match error {
        CycleScheduleRestrictionErrorV1::Cancelled => {
            CommonArticulationContinuousLayerPathErrorV1::Cancelled
        }
        CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
        }
        CycleScheduleRestrictionErrorV1::Prepare(_) => {
            CommonArticulationContinuousLayerPathErrorV1::BlockScheduleRestrictionMismatch
        }
    })
}

fn validate_complete_block_schedule_restrictions_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    schedule: &CanonicalCycleScheduleV1,
    complete: &CompleteMultiBlockPositiveLayerAuthorityV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
    if complete.parent.parent.blocks.len() != complete.blocks.len() {
        return Err(CommonArticulationContinuousLayerPathErrorV1::CanonicalBlockPartitionMismatch);
    }
    for block in &complete.parent.parent.blocks {
        checkpoint()?;
        let restricted = restrict_schedule_with_final_checkpoint_v1(
            schedule,
            geometry,
            audit,
            &block.geometry,
            &block.audit,
            block.closure.fixed_face(),
            checkpoint,
        )?;
        if restricted.certificate_binding_fingerprint_v2()
            != block.schedule.certificate_binding_fingerprint_v2()
            || restricted.graph_binding_fingerprint_v1()
                != block.schedule.graph_binding_fingerprint_v1()
        {
            return Err(
                CommonArticulationContinuousLayerPathErrorV1::BlockScheduleRestrictionMismatch,
            );
        }
    }
    Ok(())
}

fn validate_block_source_restrictions_v1(
    source: &LayerOrderSnapshot,
    blocks: &[CanonicalBlockBindingV1],
    block_sources: &[&LayerOrderSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
    if blocks.len() != block_sources.len() {
        return Err(CommonArticulationContinuousLayerPathErrorV1::BlockSourceRestrictionMismatch);
    }
    for (block, restricted) in blocks.iter().zip(block_sources) {
        checkpoint()?;
        if !layer_source_is_exact_face_restriction_v1(source, restricted, &block.faces, checkpoint)?
        {
            return Err(
                CommonArticulationContinuousLayerPathErrorV1::BlockSourceRestrictionMismatch,
            );
        }
    }
    Ok(())
}

fn layer_source_is_exact_face_restriction_v1(
    source: &LayerOrderSnapshot,
    restricted: &LayerOrderSnapshot,
    faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    let contains = |face: FaceId| {
        faces
            .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
            .is_ok()
    };
    if source.model_id != restricted.model_id
        || source.provenance != restricted.provenance
        || source.proof_summary != restricted.proof_summary
    {
        return Ok(false);
    }
    let mut expected_material_faces = Vec::new();
    for face in &source.material_faces {
        checkpoint()?;
        if contains(face.face_id) {
            expected_material_faces.push(*face);
        }
    }
    if !slice_equal_with_final_checkpoint_v1(
        &expected_material_faces,
        &restricted.material_faces,
        checkpoint,
    )? {
        return Ok(false);
    }
    let mut expected_folded_faces = Vec::new();
    for face in &source.folded_faces {
        checkpoint()?;
        if contains(face.face.face_id) {
            expected_folded_faces.push(face.clone());
        }
    }
    if !slice_equal_with_final_checkpoint_v1(
        &expected_folded_faces,
        &restricted.folded_faces,
        checkpoint,
    )? {
        return Ok(false);
    }
    let mut expected_pair_orders = Vec::new();
    for pair in &source.face_pair_orders {
        checkpoint()?;
        if contains(pair.lower_face.face_id) && contains(pair.upper_face.face_id) {
            expected_pair_orders.push(pair.clone());
        }
    }
    if !slice_equal_with_final_checkpoint_v1(
        &expected_pair_orders,
        &restricted.face_pair_orders,
        checkpoint,
    )? {
        return Ok(false);
    }
    match (
        &source.global_bottom_to_top,
        &restricted.global_bottom_to_top,
    ) {
        (Some(source), Some(restricted)) => {
            let mut expected = Vec::new();
            for face in source {
                checkpoint()?;
                if contains(face.face_id) {
                    expected.push(*face);
                }
            }
            if !slice_equal_with_final_checkpoint_v1(&expected, restricted, checkpoint)? {
                return Ok(false);
            }
        }
        (None, None) => {}
        _ => return Ok(false),
    }
    checkpoint()?;
    let mut expected_reference = source.reference_face.filter(|face| contains(face.face_id));
    if expected_reference.is_none() {
        for face in &source.material_faces {
            checkpoint()?;
            if contains(face.face_id) {
                expected_reference = Some(*face);
                break;
            }
        }
    }
    if expected_reference != restricted.reference_face {
        return Ok(false);
    }
    let mut actual_cells = restricted.overlap_cells.iter();
    for cell in &source.overlap_cells {
        checkpoint()?;
        let mut cell_is_relevant = false;
        for face in &cell.bottom_to_top_faces {
            checkpoint()?;
            if contains(*face) {
                cell_is_relevant = true;
                break;
            }
        }
        if !cell_is_relevant {
            continue;
        }
        let Some(actual) = actual_cells.next() else {
            return Ok(false);
        };
        if cell.cell_key != actual.cell_key
            || !slice_equal_with_final_checkpoint_v1(
                &cell.exact_boundary,
                &actual.exact_boundary,
                checkpoint,
            )?
        {
            return Ok(false);
        }
        let mut expected_covering = Vec::new();
        for face in &cell.covering_faces {
            checkpoint()?;
            if contains(face.face_id) {
                expected_covering.push(*face);
            }
        }
        if !slice_equal_with_final_checkpoint_v1(
            &expected_covering,
            &actual.covering_faces,
            checkpoint,
        )? {
            return Ok(false);
        }
        let mut expected_order = Vec::new();
        for face in &cell.bottom_to_top_faces {
            checkpoint()?;
            if contains(*face) {
                expected_order.push(*face);
            }
        }
        if !slice_equal_with_final_checkpoint_v1(
            &expected_order,
            &actual.bottom_to_top_faces,
            checkpoint,
        )? {
            return Ok(false);
        }
    }
    checkpoint()?;
    Ok(actual_cells.next().is_none())
}

fn slice_equal_with_final_checkpoint_v1<T: PartialEq>(
    expected: &[T],
    actual: &[T],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        checkpoint()?;
        if expected != actual {
            return Ok(false);
        }
    }
    Ok(true)
}

fn map_final_staged_error_v1(
    error: CommonArticulationBlockComposedPathErrorV1,
) -> CommonArticulationContinuousLayerPathErrorV1 {
    match error {
        CommonArticulationBlockComposedPathErrorV1::Cancelled => {
            CommonArticulationContinuousLayerPathErrorV1::Cancelled
        }
        CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
        }
        error => CommonArticulationContinuousLayerPathErrorV1::Staged(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn layer_proof_is_for_with_final_checkpoint_v1(
    proof: &GeneralMultiFaceCellTransportProofV1,
    geometry: &MaterialHingeGraphGeometry,
    source: &LayerOrderSnapshot,
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    thickness: f64,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathErrorV1>,
) -> Result<bool, CommonArticulationContinuousLayerPathErrorV1> {
    let mut unexpected_checkpoint_error = None;
    let result =
        proof.is_for_with_checkpoint_v1(geometry, source, schedule, closure, thickness, || {
            match checkpoint() {
                Ok(()) => Ok(()),
                Err(CommonArticulationContinuousLayerPathErrorV1::Cancelled) => {
                    Err(CooperativeOperationStopV1::Cancelled)
                }
                Err(CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded) => {
                    Err(CooperativeOperationStopV1::DeadlineExceeded)
                }
                Err(error) => {
                    unexpected_checkpoint_error = Some(error);
                    Err(CooperativeOperationStopV1::Cancelled)
                }
            }
        });
    if let Some(error) = unexpected_checkpoint_error {
        return Err(error);
    }
    result.map_err(map_cooperative_stop_to_final_v1)
}

fn map_cooperative_stop_to_final_v1(
    stop: CooperativeOperationStopV1,
) -> CommonArticulationContinuousLayerPathErrorV1 {
    match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationContinuousLayerPathErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
        }
    }
}

fn final_path_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationContinuousLayerPathErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../test-support/miura_cactus.rs"]
mod miura_cactus_test_support;

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet, HashSet},
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    use super::{
        COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1, CanonicalBlockBindingV1,
        CommonArticulationPoseErrorV1, CommonArticulationPoseInputV1,
        CommonArticulationPoseLimitsV1, Digest, EXACT_NINE_BLOCK_ARITY_V1,
        EXACT_TEN_BLOCK_ARITY_V1, MULTI_BLOCK_MAX_BLOCKS_V1, MULTI_BLOCK_MIN_BLOCKS_V1,
        MultiBlockClosureInputV1, MultiBlockPositiveLayerInputV1, Sha256,
        block_articulation_incidence_is_tree_v1, issue_common_articulation_pose_authority_v1,
        issue_common_articulation_pose_authority_with_control_v1,
        issue_complete_multi_block_positive_layer_authority_v1,
        issue_exact_nine_block_closure_authority_v1, issue_exact_ten_block_closure_authority_v1,
        issue_multi_block_closure_authority_v1, issue_multi_block_positive_layer_authority_v1,
        multi_block_count_supported_v1,
    };
    use crate::{
        CooperativeOperationControlV1, GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
        certify_canonical_positive_thickness_cycle_schedule_path_v1,
        certify_general_multi_face_cell_transport_v1,
    };
    use ori_core::{analyze_global_flat_foldability, analyze_local_flat_foldability};
    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
    };
    use ori_foldability::{
        GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, LayerOrderSnapshot,
    };
    use ori_kinematics::{
        CanonicalCycleScheduleV1, CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles,
        CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HalfAngleRationalEntryInputV1,
        HingeAngle, MaterialHingeGraphAudit, MaterialHingeGraphGeometry, RationalCoefficientV1,
        TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    fn block(faces: &[FaceId]) -> CanonicalBlockBindingV1 {
        let mut faces = faces.to_vec();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        CanonicalBlockBindingV1 {
            edges: Vec::new(),
            faces,
        }
    }

    type MiuraBlockFixture = (CreasePattern, Paper, Vec<EdgeId>);

    fn chain_pattern_for_cells(cells: &[(i16, i16)], namespace: ProjectId) -> MiuraBlockFixture {
        let mut points = BTreeSet::new();
        let mut incidence =
            BTreeMap::<((i16, i16), (i16, i16)), (usize, (i16, i16), (i16, i16))>::new();
        for &(x, y) in cells {
            let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
            points.extend(corners);
            for index in 0..4 {
                let start = corners[index];
                let end = corners[(index + 1) % 4];
                let key = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                incidence
                    .entry(key)
                    .and_modify(|entry| entry.0 += 1)
                    .or_insert((1, start, end));
            }
        }
        let vertex_id = |point: (i16, i16)| {
            let mut name = Vec::with_capacity(5);
            name.push(0xd1);
            name.extend_from_slice(&point.0.to_be_bytes());
            name.extend_from_slice(&point.1.to_be_bytes());
            VertexId::derive_v5(namespace, &name)
        };
        let vertices = points
            .iter()
            .map(|&point| Vertex {
                id: vertex_id(point),
                position: Point2::new(f64::from(point.0) * 20.0, f64::from(point.1) * 20.0),
            })
            .collect::<Vec<_>>();
        let mut moving = Vec::new();
        let edges = incidence
            .iter()
            .map(|(&(first, second), &(count, start, end))| {
                let mut name = Vec::with_capacity(9);
                name.push(0xd2);
                name.extend_from_slice(&first.0.to_be_bytes());
                name.extend_from_slice(&first.1.to_be_bytes());
                name.extend_from_slice(&second.0.to_be_bytes());
                name.extend_from_slice(&second.1.to_be_bytes());
                let id = EdgeId::derive_v5(namespace, &name);
                let kind = if count == 1 {
                    EdgeKind::Boundary
                } else if first.1 == second.1 {
                    moving.push(id);
                    EdgeKind::Mountain
                } else if first.1.rem_euclid(2) == 0 {
                    EdgeKind::Valley
                } else {
                    EdgeKind::Mountain
                };
                Edge {
                    id,
                    start: vertex_id(start),
                    end: vertex_id(end),
                    kind,
                }
            })
            .collect::<Vec<_>>();
        let directed = incidence
            .values()
            .filter(|(count, _, _)| *count == 1)
            .map(|(_, start, end)| (*start, *end))
            .collect::<Vec<_>>();
        let mut boundary = vec![directed[0].0];
        while boundary.len() < directed.len() {
            let cursor = *boundary.last().expect("boundary cursor");
            let next = directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("closed boundary")
                .1;
            boundary.push(next);
        }
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary.into_iter().map(vertex_id).collect(),
                thickness_mm: 0.1,
                ..Paper::default()
            },
            moving,
        )
    }

    fn miura_block_chain_v1(block_count: usize) -> (Vec<MiuraBlockFixture>, MiuraBlockFixture) {
        assert!(
            (multi_block_count_supported_v1(block_count)
                || block_count == EXACT_NINE_BLOCK_ARITY_V1
                || block_count == EXACT_TEN_BLOCK_ARITY_V1)
                && block_count >= 3
        );
        let namespace = ProjectId::new();
        let blocks = (0..block_count)
            .map(|index| {
                let x = i16::try_from(index * 2).expect("bounded block x");
                let y = if index % 2 == 0 { 0_i16 } else { -2_i16 };
                (x..=x + 2)
                    .flat_map(|x| (y..=y + 2).map(move |y| (x, y)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let combined = blocks
            .iter()
            .flat_map(|block| block.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        (
            blocks
                .iter()
                .map(|cells| chain_pattern_for_cells(cells, namespace))
                .collect(),
            chain_pattern_for_cells(&combined, namespace),
        )
    }

    #[allow(clippy::type_complexity)]
    fn prepare_complete_chain_v1(
        block_count: usize,
    ) -> (
        Vec<(
            CreasePattern,
            MaterialHingeGraphGeometry,
            MaterialHingeGraphAudit,
            Vec<EdgeId>,
            LayerOrderSnapshot,
        )>,
        Vec<(
            CanonicalCycleScheduleV1,
            ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
        )>,
        MaterialHingeGraphGeometry,
        MaterialHingeGraphGeometry,
    ) {
        let (fixtures, (live_pattern, live_paper, _)) = miura_block_chain_v1(block_count);
        let face_namespace = ProjectId::new();
        let mut prepared = fixtures
            .into_iter()
            .map(|(pattern, paper, moving)| {
                let topology = analyze_faces(FaceExtractionInput {
                    identity_namespace: face_namespace,
                    source_revision: 1,
                    paper: &paper,
                    pattern: &pattern,
                })
                .snapshot
                .expect("multi-block topology");
                let geometry = MaterialHingeGraphGeometry::prepare(
                    &pattern,
                    &paper,
                    &topology,
                    TreeKinematicsLimits::default(),
                )
                .expect("multi-block geometry");
                let audit =
                    MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                        .expect("multi-block audit");
                let local = analyze_local_flat_foldability(&paper, &pattern);
                let source = analyze_global_flat_foldability(
                    GlobalFlatFoldabilityInput::current_with_geometry(
                        face_namespace,
                        &paper,
                        &pattern,
                        &topology,
                        &local,
                    ),
                    GlobalFlatFoldabilityLimits::default(),
                )
                .expect("multi-block flat foldability")
                .layer_order()
                .expect("multi-block layer order")
                .clone();
                (pattern, geometry, audit, moving, source)
            })
            .collect::<Vec<_>>();
        prepared.sort_unstable_by_key(|(_, geometry, _, _, _)| {
            geometry
                .hinges()
                .iter()
                .map(|hinge| hinge.edge().canonical_bytes())
                .min()
                .expect("non-empty block")
        });
        let block_faces = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| geometry.face_ids().to_vec())
            .collect::<Vec<_>>();
        let fixed_faces = block_faces
            .iter()
            .enumerate()
            .map(|(index, faces)| {
                faces
                    .iter()
                    .copied()
                    .find(|face| {
                        block_faces
                            .iter()
                            .enumerate()
                            .any(|(other, candidate)| other != index && candidate.contains(face))
                    })
                    .expect("shared articulation")
            })
            .collect::<Vec<_>>();
        let scheduled = prepared
            .iter()
            .enumerate()
            .map(|(index, (pattern, geometry, audit, moving, _))| {
                let fixed = fixed_faces[index];
                let row = moving
                    .iter()
                    .map(|edge| {
                        let edge = pattern
                            .edges
                            .iter()
                            .find(|item| item.id == *edge)
                            .expect("moving edge");
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == edge.start)
                            .expect("moving start")
                            .position
                            .y
                            .to_bits()
                    })
                    .min()
                    .expect("moving row");
                let active = moving
                    .iter()
                    .filter(|edge| {
                        let edge = pattern
                            .edges
                            .iter()
                            .find(|item| item.id == **edge)
                            .expect("active edge");
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == edge.start)
                            .expect("active start")
                            .position
                            .y
                            .to_bits()
                            == row
                    })
                    .copied()
                    .collect::<HashSet<_>>();
                let entries = geometry
                    .hinges()
                    .iter()
                    .map(|hinge| {
                        let moves = active.contains(&hinge.edge());
                        HalfAngleRationalEntryInputV1 {
                            edge: hinge.edge(),
                            u_domain: [
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 1,
                                    denominator: 1,
                                },
                            ],
                            numerator_power_coefficients: vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                            ],
                            denominator_power_coefficients: vec![RationalCoefficientV1 {
                                numerator: if moves { 64 } else { 1 },
                                denominator: 1,
                            }],
                        }
                    })
                    .collect();
                let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    geometry,
                    audit,
                    fixed,
                    entries,
                    CycleScheduleLimitsV1::default(),
                )
                .expect("multi-block schedule");
                let closure = geometry
                    .prove_dyadic_schedule_closure_v1(
                        audit,
                        fixed,
                        &schedule,
                        1.0e-8,
                        DyadicIntervalClosureLimitsV1 {
                            max_depth: 8,
                            max_leaves: 256,
                            max_work: 1_000_000,
                            schedule_limits: CycleScheduleLimitsV1::default(),
                        },
                    )
                    .expect("multi-block closure");
                (schedule, closure)
            })
            .collect();
        let live_topology = analyze_faces(FaceExtractionInput {
            identity_namespace: face_namespace,
            source_revision: 1,
            paper: &live_paper,
            pattern: &live_pattern,
        })
        .snapshot
        .expect("live multi-block topology");
        let live_geometry = MaterialHingeGraphGeometry::prepare(
            &live_pattern,
            &live_paper,
            &live_topology,
            TreeKinematicsLimits::default(),
        )
        .expect("live multi-block geometry");
        let detached_live_geometry = MaterialHingeGraphGeometry::prepare(
            &live_pattern,
            &live_paper,
            &live_topology,
            TreeKinematicsLimits::default(),
        )
        .expect("detached live multi-block geometry");
        (prepared, scheduled, live_geometry, detached_live_geometry)
    }

    fn completeness_report_v1(
        geometry: &MaterialHingeGraphGeometry,
        face_blocks: &[Vec<FaceId>],
        hinge_blocks: &[Vec<EdgeId>],
    ) -> Option<super::BlockUnionCompletenessGapReportV1> {
        let inputs = face_blocks
            .iter()
            .zip(hinge_blocks)
            .map(|(faces, hinges)| super::BlockUnionCompletenessInputV1 { faces, hinges })
            .collect::<Vec<_>>();
        match inputs.len() {
            EXACT_NINE_BLOCK_ARITY_V1 => {
                super::diagnose_exact_nine_block_union_completeness_v1(geometry, &inputs)
            }
            EXACT_TEN_BLOCK_ARITY_V1 => {
                super::diagnose_exact_ten_block_union_completeness_v1(geometry, &inputs)
            }
            _ => super::diagnose_block_union_completeness_v1(geometry, &inputs),
        }
    }

    #[test]
    fn block_articulation_incidence_accepts_chain_and_shared_face_star() {
        let [a, b, c, d] = std::array::from_fn(|_| FaceId::new());
        assert!(block_articulation_incidence_is_tree_v1(&[
            block(&[a, b]),
            block(&[b, c]),
            block(&[c, d]),
        ]));
        assert!(block_articulation_incidence_is_tree_v1(&[
            block(&[a, b]),
            block(&[a, c]),
            block(&[a, d]),
            block(&[a, FaceId::new()]),
        ]));
    }

    #[test]
    fn block_intersection_rejects_an_isolated_block() {
        let [a, b, c, d] = std::array::from_fn(|_| FaceId::new());
        assert!(!block_articulation_incidence_is_tree_v1(&[
            block(&[a, b]),
            block(&[b, c]),
            block(&[d]),
        ]));
    }

    #[test]
    fn block_intersection_rejects_an_articulation_cycle() {
        let [a, b, c] = std::array::from_fn(|_| FaceId::new());
        assert!(!block_articulation_incidence_is_tree_v1(&[
            block(&[a, b]),
            block(&[b, c]),
            block(&[c, a]),
        ]));
    }

    #[test]
    fn bounded_multi_block_count_fails_closed() {
        assert!(!multi_block_count_supported_v1(
            MULTI_BLOCK_MIN_BLOCKS_V1 - 1
        ));
        assert!(multi_block_count_supported_v1(MULTI_BLOCK_MIN_BLOCKS_V1));
        assert!(multi_block_count_supported_v1(MULTI_BLOCK_MAX_BLOCKS_V1));
        assert!(!multi_block_count_supported_v1(
            MULTI_BLOCK_MAX_BLOCKS_V1 + 1
        ));
    }

    #[test]
    fn submitted_set_scopes_have_disjoint_arity_and_binding_domains() {
        let generic = super::MultiBlockAdmissionScopeV1::GenericSubmitted2To8;
        let exact_nine = super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet;
        let exact_ten = super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet;
        assert!(generic.admits_block_count_v1(2));
        assert!(generic.admits_block_count_v1(8));
        assert!(!generic.admits_block_count_v1(9));
        assert!(!generic.admits_block_count_v1(10));
        assert!(!exact_nine.admits_block_count_v1(8));
        assert!(exact_nine.admits_block_count_v1(9));
        assert!(!exact_nine.admits_block_count_v1(10));
        assert!(!exact_ten.admits_block_count_v1(9));
        assert!(exact_ten.admits_block_count_v1(10));
        assert!(!exact_ten.admits_block_count_v1(11));

        assert_eq!(generic.closure_domain_tag_v1(), b"closure_v1");
        assert_eq!(generic.positive_layer_domain_tag_v1(), None);
        assert_eq!(generic.complete_live_domain_tag_v1(), None);
        assert_eq!(
            exact_nine.closure_domain_tag_v1(),
            b"exact-nine-submitted-set-closure-v1",
        );
        assert_eq!(
            exact_nine.positive_layer_domain_tag_v1(),
            Some(b"exact-nine-submitted-set-positive-layer-v1".as_slice()),
        );
        assert_eq!(
            exact_nine.complete_live_domain_tag_v1(),
            Some(b"exact-nine-submitted-set-complete-live-v1".as_slice()),
        );
        let effective_domains = [
            generic.closure_domain_tag_v1(),
            exact_nine.closure_domain_tag_v1(),
            exact_nine
                .positive_layer_domain_tag_v1()
                .expect("exact-nine positive-layer domain"),
            exact_nine
                .complete_live_domain_tag_v1()
                .expect("exact-nine complete-live domain"),
            exact_ten.closure_domain_tag_v1(),
            exact_ten
                .positive_layer_domain_tag_v1()
                .expect("exact-ten positive-layer domain"),
            exact_ten
                .complete_live_domain_tag_v1()
                .expect("exact-ten complete-live domain"),
        ];
        for (index, domain) in effective_domains.iter().enumerate() {
            assert!(
                effective_domains[index + 1..]
                    .iter()
                    .all(|other| domain != other)
            );
        }

        let closure_domain_probe = |scope: super::MultiBlockAdmissionScopeV1| {
            let mut hash = Sha256::new();
            hash.update(super::MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
            hash.update(scope.closure_domain_tag_v1());
            hash.update(b"identical-owned-input-probe-v1");
            <[u8; 32]>::from(hash.finalize())
        };
        assert_ne!(
            closure_domain_probe(generic),
            closure_domain_probe(exact_nine)
        );
        assert_ne!(
            closure_domain_probe(generic),
            closure_domain_probe(exact_ten)
        );
        assert_ne!(
            closure_domain_probe(exact_nine),
            closure_domain_probe(exact_ten)
        );
    }

    #[test]
    fn exact_nine_submitted_set_companions_preserve_generic_eight_block_boundary() {
        let (prepared, scheduled, live_geometry, _) = prepare_complete_chain_v1(9);
        let closure_inputs = || {
            prepared
                .iter()
                .zip(&scheduled)
                .map(
                    |((_, geometry, audit, _, _), (schedule, closure))| MultiBlockClosureInputV1 {
                        geometry,
                        audit,
                        schedule,
                        closure,
                    },
                )
                .collect::<Vec<_>>()
        };
        assert!(
            issue_multi_block_closure_authority_v1(closure_inputs(), 0.1, [0x91; 32]).is_none(),
            "the generic submitted-set issuer remains capped at eight blocks",
        );
        assert!(
            super::issue_exact_nine_block_closure_authority_v1(
                closure_inputs().into_iter().take(8).collect(),
                0.1,
                [0x91; 32],
            )
            .is_none(),
            "the companion issuer admits exactly nine blocks only",
        );
        let ten_closure_inputs = (0..10)
            .map(|index| {
                let (_, geometry, audit, _, _) = &prepared[index % prepared.len()];
                let (schedule, closure) = &scheduled[index % scheduled.len()];
                MultiBlockClosureInputV1 {
                    geometry,
                    audit,
                    schedule,
                    closure,
                }
            })
            .collect();
        assert!(
            super::issue_exact_nine_block_closure_authority_v1(
                ten_closure_inputs,
                0.1,
                [0x91; 32],
            )
            .is_none(),
            "the companion issuer rejects ten blocks",
        );
        let authority =
            super::issue_exact_nine_block_closure_authority_v1(closure_inputs(), 0.1, [0x91; 32])
                .expect("exact-nine submitted-set authority");
        assert_eq!(authority.block_count_v1(), 9);
        assert_eq!(
            authority.scope,
            super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
        );
        assert_ne!(authority.binding_fingerprint_v1(), [0; 32]);

        let block_faces = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| geometry.face_ids().to_vec())
            .collect::<Vec<_>>();
        let block_hinges = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| {
                geometry
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let union_inputs = block_faces
            .iter()
            .zip(&block_hinges)
            .map(|(faces, hinges)| super::BlockUnionCompletenessInputV1 { faces, hinges })
            .collect::<Vec<_>>();
        assert!(
            super::diagnose_block_union_completeness_v1(&live_geometry, &union_inputs).is_none(),
            "the generic union diagnosis remains capped at eight blocks",
        );
        assert!(
            super::diagnose_exact_nine_block_union_completeness_v1(
                &live_geometry,
                &union_inputs[..8],
            )
            .is_none(),
            "the companion union diagnosis admits exactly nine blocks only",
        );
        let ten_union_inputs = (0..10)
            .map(|index| super::BlockUnionCompletenessInputV1 {
                faces: &block_faces[index % block_faces.len()],
                hinges: &block_hinges[index % block_hinges.len()],
            })
            .collect::<Vec<_>>();
        assert!(
            super::diagnose_exact_nine_block_union_completeness_v1(
                &live_geometry,
                &ten_union_inputs,
            )
            .is_none(),
            "the companion union diagnosis rejects ten blocks",
        );
        let report =
            super::diagnose_exact_nine_block_union_completeness_v1(&live_geometry, &union_inputs)
                .expect("exact-nine live-union report");
        assert_eq!(
            report.scope,
            super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
        );
        assert!(report.exact_live_union_observed());
        assert!(!report.authorizes_multi_block_composition());
        assert!(!report.authorizes_project_mutation());
    }

    #[test]
    fn exact_ten_submitted_set_companions_are_exact_and_non_authorizing() {
        let (prepared, scheduled, live_geometry, _) = prepare_complete_chain_v1(10);
        let closure_inputs = || {
            prepared
                .iter()
                .zip(&scheduled)
                .map(
                    |((_, geometry, audit, _, _), (schedule, closure))| MultiBlockClosureInputV1 {
                        geometry,
                        audit,
                        schedule,
                        closure,
                    },
                )
                .collect::<Vec<_>>()
        };
        assert!(
            issue_multi_block_closure_authority_v1(closure_inputs(), 0.1, [0xa1; 32]).is_none(),
            "the generic submitted-set issuer remains capped at eight blocks",
        );
        assert!(
            issue_exact_nine_block_closure_authority_v1(closure_inputs(), 0.1, [0xa1; 32],)
                .is_none(),
            "the exact-nine issuer rejects the exact-ten submitted set",
        );
        assert!(
            issue_exact_ten_block_closure_authority_v1(
                closure_inputs().into_iter().take(9).collect(),
                0.1,
                [0xa1; 32],
            )
            .is_none(),
            "the exact-ten issuer rejects nine blocks",
        );
        let mut eleven_closure_inputs = closure_inputs();
        let (_, geometry, audit, _, _) = &prepared[0];
        let (schedule, closure) = &scheduled[0];
        eleven_closure_inputs.push(MultiBlockClosureInputV1 {
            geometry,
            audit,
            schedule,
            closure,
        });
        assert!(
            issue_exact_ten_block_closure_authority_v1(eleven_closure_inputs, 0.1, [0xa1; 32],)
                .is_none(),
            "the exact-ten issuer rejects eleven blocks",
        );
        let authority =
            issue_exact_ten_block_closure_authority_v1(closure_inputs(), 0.1, [0xa1; 32])
                .expect("exact-ten submitted-set authority");
        assert_eq!(authority.block_count_v1(), 10);
        assert_eq!(
            authority.scope,
            super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
        );
        assert_ne!(authority.binding_fingerprint_v1(), [0; 32]);

        let block_faces = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| geometry.face_ids().to_vec())
            .collect::<Vec<_>>();
        let block_hinges = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| {
                geometry
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let union_inputs = block_faces
            .iter()
            .zip(&block_hinges)
            .map(|(faces, hinges)| super::BlockUnionCompletenessInputV1 { faces, hinges })
            .collect::<Vec<_>>();
        assert!(
            super::diagnose_block_union_completeness_v1(&live_geometry, &union_inputs).is_none()
        );
        assert!(
            super::diagnose_exact_nine_block_union_completeness_v1(&live_geometry, &union_inputs,)
                .is_none()
        );
        assert!(
            super::diagnose_exact_ten_block_union_completeness_v1(
                &live_geometry,
                &union_inputs[..9],
            )
            .is_none()
        );
        let mut eleven_union_inputs = block_faces
            .iter()
            .zip(&block_hinges)
            .map(|(faces, hinges)| super::BlockUnionCompletenessInputV1 { faces, hinges })
            .collect::<Vec<_>>();
        eleven_union_inputs.push(super::BlockUnionCompletenessInputV1 {
            faces: &block_faces[0],
            hinges: &block_hinges[0],
        });
        assert!(
            super::diagnose_exact_ten_block_union_completeness_v1(
                &live_geometry,
                &eleven_union_inputs,
            )
            .is_none()
        );
        let report =
            super::diagnose_exact_ten_block_union_completeness_v1(&live_geometry, &union_inputs)
                .expect("exact-ten live-union report");
        assert_eq!(
            report.scope,
            super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
        );
        assert!(report.exact_live_union_observed());
        assert!(!report.authorizes_multi_block_composition());
        assert!(!report.authorizes_project_mutation());
    }

    #[test]
    fn common_articulation_adapter_issues_and_maps_cooperative_stops() {
        let namespace = ProjectId::new();
        let (pattern, paper, _) = chain_pattern_for_cells(&[(0, 0), (1, 0), (2, 0)], namespace);
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("three-cell topology");
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .expect("three-cell geometry");
        let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
            .expect("three-cell audit");
        let angles = CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero hinge angle"))
                .collect(),
        )
        .expect("canonical zero angles");
        let pose = geometry
            .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
            .expect("closed three-cell pose");
        let decomposition = geometry
            .decompose_canonical_edge_blocks_v1(
                &audit,
                CanonicalEdgeBlockLimitsV1 {
                    max_blocks: 8,
                    max_faces_per_block: 32,
                    max_hinges_per_block: 32,
                },
            )
            .expect("canonical articulation decomposition");
        let input = CommonArticulationPoseInputV1 {
            geometry: &geometry,
            pose: &pose,
            decomposition: &decomposition,
            paper_thickness_mm: paper.thickness_mm,
            limits: CommonArticulationPoseLimitsV1::default(),
        };
        let authority = issue_common_articulation_pose_authority_v1(input)
            .expect("collision adapter authority");
        assert!(authority.revalidate_v1(input).is_ok());
        assert!(!authority.authorizes_continuous_motion());
        assert!(!authority.authorizes_collision_clearance());
        assert!(!authority.authorizes_project_mutation());
        assert!(!authority.authorizes_apply());
        assert!(!authority.authorizes_viewer());

        let cancelled = AtomicBool::new(true);
        assert!(matches!(
            issue_common_articulation_pose_authority_with_control_v1(
                input,
                &CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            ),
            Err(CommonArticulationPoseErrorV1::Cancelled)
        ));
        assert!(matches!(
            issue_common_articulation_pose_authority_with_control_v1(
                input,
                &CooperativeOperationControlV1::new(
                    None,
                    Instant::now() - Duration::from_millis(1),
                ),
            ),
            Err(CommonArticulationPoseErrorV1::DeadlineExceeded)
        ));
    }

    #[test]
    fn submitted_three_block_tree_authority_revalidates_and_rejects_bound_tampering() {
        let (fixtures, _) =
            super::miura_cactus_test_support::three_three_by_three_miura_blocks_with_document();
        let namespace = ProjectId::new();
        let mut prepared = fixtures
            .map(|(pattern, paper, moving)| {
                let topology = analyze_faces(FaceExtractionInput {
                    identity_namespace: namespace,
                    source_revision: 1,
                    paper: &paper,
                    pattern: &pattern,
                })
                .snapshot
                .expect("three-block topology");
                let geometry = MaterialHingeGraphGeometry::prepare(
                    &pattern,
                    &paper,
                    &topology,
                    TreeKinematicsLimits::default(),
                )
                .expect("three-block geometry");
                let audit =
                    MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                        .expect("three-block audit");
                let local = analyze_local_flat_foldability(&paper, &pattern);
                let source = analyze_global_flat_foldability(
                    GlobalFlatFoldabilityInput::current_with_geometry(
                        namespace, &paper, &pattern, &topology, &local,
                    ),
                    GlobalFlatFoldabilityLimits::default(),
                )
                .expect("three-block flat foldability")
                .layer_order()
                .expect("three-block layer order")
                .clone();
                (pattern, geometry, audit, moving, source)
            })
            .into_iter()
            .collect::<Vec<_>>();
        prepared.sort_unstable_by_key(|(_, geometry, _, _, _)| {
            geometry
                .hinges()
                .iter()
                .map(|hinge| hinge.edge().canonical_bytes())
                .min()
                .expect("non-empty block")
        });
        for block_count in [4_usize, 8] {
            let geometry = &prepared[0].1;
            let mut face_blocks = vec![Vec::new(); block_count];
            let mut hinge_blocks = vec![Vec::new(); block_count];
            for (index, face) in geometry.face_ids().iter().copied().enumerate() {
                face_blocks[index % block_count].push(face);
            }
            for (index, hinge) in geometry.hinges().iter().enumerate() {
                hinge_blocks[index % block_count].push(hinge.edge());
            }
            for index in 1..block_count {
                let articulation = face_blocks[index - 1][0];
                face_blocks[index].push(articulation);
            }
            let inputs = (0..block_count)
                .map(|index| super::BlockUnionCompletenessInputV1 {
                    faces: &face_blocks[index],
                    hinges: &hinge_blocks[index],
                })
                .collect::<Vec<_>>();
            let report = super::diagnose_block_union_completeness_v1(geometry, &inputs)
                .expect("bounded completeness report");
            assert!(report.is_for(geometry));
            assert!(report.exact_live_union_observed());
            assert!(!report.authorizes_multi_block_composition());
            assert!(!report.authorizes_project_mutation());
            let mut omitted_faces = face_blocks.clone();
            let omitted = omitted_faces[block_count - 1]
                .iter()
                .position(|face| {
                    !omitted_faces[..block_count - 1]
                        .iter()
                        .any(|b| b.contains(face))
                })
                .unwrap();
            omitted_faces[block_count - 1].remove(omitted);
            let omitted_inputs = (0..block_count)
                .map(|index| super::BlockUnionCompletenessInputV1 {
                    faces: &omitted_faces[index],
                    hinges: &hinge_blocks[index],
                })
                .collect::<Vec<_>>();
            assert!(
                !super::diagnose_block_union_completeness_v1(geometry, &omitted_inputs)
                    .unwrap()
                    .exact_live_union_observed()
            );
            let mut extra_faces = face_blocks.clone();
            extra_faces[0].push(FaceId::new());
            let extra_inputs = (0..block_count)
                .map(|index| super::BlockUnionCompletenessInputV1 {
                    faces: &extra_faces[index],
                    hinges: &hinge_blocks[index],
                })
                .collect::<Vec<_>>();
            assert!(
                !super::diagnose_block_union_completeness_v1(geometry, &extra_inputs)
                    .unwrap()
                    .exact_live_union_observed()
            );
            let mut duplicate_hinges = hinge_blocks.clone();
            let duplicate_hinge = duplicate_hinges[1][0];
            duplicate_hinges[0].push(duplicate_hinge);
            let hinge_inputs = (0..block_count)
                .map(|index| super::BlockUnionCompletenessInputV1 {
                    faces: &face_blocks[index],
                    hinges: &duplicate_hinges[index],
                })
                .collect::<Vec<_>>();
            assert!(
                !super::diagnose_block_union_completeness_v1(geometry, &hinge_inputs)
                    .unwrap()
                    .exact_live_union_observed()
            );
            let duplicate = face_blocks[2][0];
            face_blocks[0].push(duplicate);
            let tampered = (0..block_count)
                .map(|index| super::BlockUnionCompletenessInputV1 {
                    faces: &face_blocks[index],
                    hinges: &hinge_blocks[index],
                })
                .collect::<Vec<_>>();
            assert!(
                !super::diagnose_block_union_completeness_v1(geometry, &tampered)
                    .expect("tamper gap report")
                    .exact_live_union_observed()
            );
        }
        let empty = Vec::<super::BlockUnionCompletenessInputV1<'_>>::new();
        assert!(super::diagnose_block_union_completeness_v1(&prepared[0].1, &empty).is_none());
        let sixteen = (0..16)
            .map(|_| super::BlockUnionCompletenessInputV1 {
                faces: &[],
                hinges: &[],
            })
            .collect::<Vec<_>>();
        assert!(super::diagnose_block_union_completeness_v1(&prepared[0].1, &sixteen).is_none());
        let oversized_faces = vec![FaceId::new(); super::BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1];
        let oversized = [
            super::BlockUnionCompletenessInputV1 {
                faces: &oversized_faces,
                hinges: &[EdgeId::new()],
            },
            super::BlockUnionCompletenessInputV1 {
                faces: &[FaceId::new()],
                hinges: &[EdgeId::new()],
            },
        ];
        assert!(super::diagnose_block_union_completeness_v1(&prepared[0].1, &oversized).is_none());
        let block_faces = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| geometry.face_ids().to_vec());
        let block_faces = block_faces.collect::<Vec<_>>();
        let fixed_faces = block_faces
            .iter()
            .enumerate()
            .map(|(index, faces)| {
                faces
                    .iter()
                    .copied()
                    .find(|face| {
                        block_faces
                            .iter()
                            .enumerate()
                            .any(|(other, candidate)| other != index && candidate.contains(face))
                    })
                    .expect("shared articulation")
            })
            .collect::<Vec<_>>();
        let scheduled = prepared
            .iter()
            .enumerate()
            .map(|(index, (pattern, geometry, audit, moving, _))| {
                let fixed = fixed_faces[index];
                let row = moving
                    .iter()
                    .map(|edge| {
                        let edge = pattern.edges.iter().find(|item| item.id == *edge).unwrap();
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == edge.start)
                            .unwrap()
                            .position
                            .y
                            .to_bits()
                    })
                    .min()
                    .expect("moving row");
                let active = moving
                    .iter()
                    .filter(|edge| {
                        let edge = pattern.edges.iter().find(|item| item.id == **edge).unwrap();
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == edge.start)
                            .unwrap()
                            .position
                            .y
                            .to_bits()
                            == row
                    })
                    .copied()
                    .collect::<std::collections::HashSet<EdgeId>>();
                let entries = geometry
                    .hinges()
                    .iter()
                    .map(|hinge| {
                        let moves = active.contains(&hinge.edge());
                        HalfAngleRationalEntryInputV1 {
                            edge: hinge.edge(),
                            u_domain: [
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 1,
                                    denominator: 1,
                                },
                            ],
                            numerator_power_coefficients: vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                            ],
                            denominator_power_coefficients: vec![RationalCoefficientV1 {
                                numerator: if moves { 64 } else { 1 },
                                denominator: 1,
                            }],
                        }
                    })
                    .collect();
                let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    geometry,
                    audit,
                    fixed,
                    entries,
                    CycleScheduleLimitsV1::default(),
                )
                .expect("three-block schedule");
                let closure = geometry
                    .prove_dyadic_schedule_closure_v1(
                        audit,
                        fixed,
                        &schedule,
                        1.0e-8,
                        DyadicIntervalClosureLimitsV1 {
                            max_depth: 8,
                            max_leaves: 256,
                            max_work: 1_000_000,
                            schedule_limits: CycleScheduleLimitsV1::default(),
                        },
                    )
                    .expect("three-block closure");
                (schedule, closure)
            })
            .collect::<Vec<_>>();
        let thickness = 0.1;
        let issuer_context = [0x41; 32];
        let layer_fingerprint = [0x42; 32];
        let closure_input = |index: usize| {
            let (_, geometry, audit, _, _) = &prepared[index];
            let (schedule, closure) = &scheduled[index];
            MultiBlockClosureInputV1 {
                geometry,
                audit,
                schedule,
                closure,
            }
        };
        assert!(
            issue_multi_block_closure_authority_v1(
                vec![closure_input(0), closure_input(0), closure_input(2)],
                thickness,
                issuer_context,
            )
            .is_none()
        );
        assert!(
            issue_multi_block_closure_authority_v1(
                vec![closure_input(0), closure_input(1), closure_input(2)],
                0.0,
                issuer_context,
            )
            .is_none()
        );
        assert!(
            issue_multi_block_closure_authority_v1(
                vec![closure_input(0), closure_input(1), closure_input(2)],
                thickness,
                [0; 32],
            )
            .is_none()
        );
        let parent = issue_multi_block_closure_authority_v1(
            prepared
                .iter()
                .zip(&scheduled)
                .map(
                    |((_, geometry, audit, _, _), (schedule, closure))| MultiBlockClosureInputV1 {
                        geometry,
                        audit,
                        schedule,
                        closure,
                    },
                )
                .collect(),
            thickness,
            issuer_context,
        )
        .expect("three-block closure authority");
        let proofs = prepared
            .iter()
            .zip(&scheduled)
            .map(|((_, geometry, audit, _, source), (schedule, closure))| {
                let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
                    geometry,
                    audit,
                    closure.fixed_face(),
                    schedule,
                    closure,
                    thickness,
                    32,
                )
                .expect("positive path");
                let layer =
                    certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                        geometry,
                        audit,
                        source,
                        schedule,
                        closure,
                        positive_continuous: &positive,
                        paper_thickness_mm: thickness,
                        tolerance: 1.0e-8,
                        limits: GeneralCellTransportLimitsV1 {
                            max_transitions: closure.leaves().len() + 1,
                            max_cells: 1_000_000,
                            max_layer_records: 1_000_000,
                            max_boundary_samples: 1_000_000,
                        },
                    })
                    .expect("layer transport");
                (positive, layer)
            })
            .collect::<Vec<_>>();
        let authority = issue_multi_block_positive_layer_authority_v1(
            parent,
            prepared
                .iter()
                .zip(proofs)
                .map(|((_, geometry, _, _, source), (positive, layer))| {
                    MultiBlockPositiveLayerInputV1 {
                        geometry,
                        source,
                        positive,
                        layer,
                    }
                })
                .collect(),
            layer_fingerprint,
        )
        .expect("three-block positive layer authority");
        let sources = prepared
            .iter()
            .map(|(_, _, _, _, source)| source)
            .collect::<Vec<&LayerOrderSnapshot>>();
        assert!(authority.revalidates_v1(&sources, thickness, issuer_context, layer_fingerprint,));
        assert!(!authority.revalidates_v1(
            &sources,
            thickness + 0.1,
            issuer_context,
            layer_fingerprint,
        ));
        assert!(!authority.revalidates_v1(&sources, thickness, [0x40; 32], layer_fingerprint,));
        assert!(!authority.revalidates_v1(&sources, thickness, issuer_context, [0x43; 32],));
        let mut reordered_sources = sources.clone();
        reordered_sources.swap(0, 1);
        assert!(!authority.revalidates_v1(
            &reordered_sources,
            thickness,
            issuer_context,
            layer_fingerprint,
        ));
        let mut altered_source = (*sources[0]).clone();
        altered_source.material_faces.pop();
        let altered_sources = vec![&altered_source, sources[1], sources[2]];
        assert!(!authority.revalidates_v1(
            &altered_sources,
            thickness,
            issuer_context,
            layer_fingerprint,
        ));
        let mut target = prepared
            .iter()
            .zip(&scheduled)
            .flat_map(|((_, geometry, _, _, _), (schedule, _))| {
                schedule
                    .evaluate(1.0)
                    .unwrap()
                    .as_slice()
                    .to_vec()
                    .into_iter()
                    .map(move |angle| {
                        debug_assert!(
                            geometry
                                .hinges()
                                .iter()
                                .any(|hinge| hinge.edge() == angle.edge())
                        );
                        (angle.edge(), angle.angle_degrees())
                    })
            })
            .collect::<Vec<_>>();
        assert!(authority.target_angles_match_v1(&target));
        let mut missing_target = target.clone();
        missing_target.pop();
        assert!(!authority.target_angles_match_v1(&missing_target));
        let mut duplicate_target = target.clone();
        duplicate_target[1] = duplicate_target[0];
        assert!(!authority.target_angles_match_v1(&duplicate_target));
        target[0].1 = f64::from_bits(target[0].1.to_bits() ^ 1);
        assert!(!authority.target_angles_match_v1(&target));
        let mut authority = authority;
        authority.binding[0] ^= 1;
        assert!(!authority.revalidates_v1(&sources, thickness, issuer_context, layer_fingerprint,));
    }

    fn direct_closure_binding_v1(
        authority: &super::MultiBlockClosureAuthorityV1,
        domain_tag: &[u8],
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(super::MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
        hash.update(domain_tag);
        hash.update(authority.thickness_bits.to_le_bytes());
        hash.update(authority.issuer_context);
        for block in &authority.blocks {
            hash.update(block.schedule.graph_binding_fingerprint_v1());
            hash.update(block.schedule.certificate_binding_fingerprint_v2());
            hash.update(block.closure.partition_binding_fingerprint_v2());
            hash.update((block.edges.len() as u64).to_le_bytes());
            for edge in &block.edges {
                hash.update(edge.canonical_bytes());
            }
            for face in &block.faces {
                hash.update(face.canonical_bytes());
            }
        }
        hash.finalize().into()
    }

    fn direct_positive_layer_binding_v1(
        authority: &super::MultiBlockPositiveLayerAuthorityV1,
        domain_tag: Option<&[u8]>,
    ) -> [u8; 32] {
        let mut records = authority
            .layer
            .iter()
            .map(|proof| {
                (
                    proof.target_order_hash(),
                    proof.paper_thickness_mm().to_bits(),
                    proof.transition_hashes().len(),
                    proof.pair_order_count(),
                )
            })
            .collect::<Vec<_>>();
        records.sort_unstable();
        let mut hash = Sha256::new();
        hash.update(super::MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
        if let Some(domain_tag) = domain_tag {
            hash.update(domain_tag);
        }
        hash.update(authority.parent.binding);
        hash.update(authority.articulation_layer_fingerprint);
        for (target, thickness, transitions, pairs) in records {
            hash.update(target);
            hash.update(thickness.to_le_bytes());
            hash.update((transitions as u64).to_le_bytes());
            hash.update((pairs as u64).to_le_bytes());
        }
        hash.finalize().into()
    }

    fn direct_complete_binding_v1(
        authority: &super::CompleteMultiBlockPositiveLayerAuthorityV1,
        domain_tag: Option<&[u8]>,
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
        if let Some(domain_tag) = domain_tag {
            hash.update(domain_tag);
        }
        hash.update(authority.parent.binding);
        hash.update((authority.live_faces.len() as u64).to_le_bytes());
        for face in &authority.live_faces {
            hash.update(face.canonical_bytes());
        }
        hash.update((authority.live_hinges.len() as u64).to_le_bytes());
        for hinge in &authority.live_hinges {
            hash.update(hinge.canonical_bytes());
        }
        hash.update((authority.blocks.len() as u64).to_le_bytes());
        for block in &authority.blocks {
            hash.update((block.faces.len() as u64).to_le_bytes());
            for face in &block.faces {
                hash.update(face.canonical_bytes());
            }
            hash.update((block.edges.len() as u64).to_le_bytes());
            for edge in &block.edges {
                hash.update(edge.canonical_bytes());
            }
        }
        hash.finalize().into()
    }

    struct ExpectedScopeDomainsV1 {
        scope: super::MultiBlockAdmissionScopeV1,
        closure: &'static [u8],
        positive: Option<&'static [u8]>,
        complete: Option<&'static [u8]>,
    }

    fn expected_scope_domains_v1(block_count: usize) -> ExpectedScopeDomainsV1 {
        match block_count {
            EXACT_NINE_BLOCK_ARITY_V1 => ExpectedScopeDomainsV1 {
                scope: super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
                closure: b"exact-nine-submitted-set-closure-v1",
                positive: Some(b"exact-nine-submitted-set-positive-layer-v1"),
                complete: Some(b"exact-nine-submitted-set-complete-live-v1"),
            },
            EXACT_TEN_BLOCK_ARITY_V1 => ExpectedScopeDomainsV1 {
                scope: super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
                closure: b"exact-ten-submitted-set-closure-v1",
                positive: Some(b"exact-ten-submitted-set-positive-layer-v1"),
                complete: Some(b"exact-ten-submitted-set-complete-live-v1"),
            },
            count if multi_block_count_supported_v1(count) => ExpectedScopeDomainsV1 {
                scope: super::MultiBlockAdmissionScopeV1::GenericSubmitted2To8,
                closure: b"closure_v1",
                positive: None,
                complete: None,
            },
            _ => panic!("unsupported complete multi-block arity: {block_count}"),
        }
    }

    fn assert_complete_live_multi_block_authority_v1(block_count: usize) {
        let (prepared, scheduled, live_geometry, detached_live_geometry) =
            prepare_complete_chain_v1(block_count);
        assert_eq!(live_geometry, detached_live_geometry);
        assert!(!live_geometry.same_instance(&detached_live_geometry));
        let thickness = 0.1;
        let issuer_context = [0x51; 32];
        let layer_fingerprint = [0x52; 32];
        let domains = expected_scope_domains_v1(block_count);
        let expected_scope = domains.scope;
        let closure_inputs = prepared
            .iter()
            .zip(&scheduled)
            .map(
                |((_, geometry, audit, _, _), (schedule, closure))| MultiBlockClosureInputV1 {
                    geometry,
                    audit,
                    schedule,
                    closure,
                },
            )
            .collect();
        let parent = match expected_scope {
            super::MultiBlockAdmissionScopeV1::GenericSubmitted2To8 => {
                issue_multi_block_closure_authority_v1(closure_inputs, thickness, issuer_context)
            }
            super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet => {
                issue_exact_nine_block_closure_authority_v1(
                    closure_inputs,
                    thickness,
                    issuer_context,
                )
            }
            super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet => {
                issue_exact_ten_block_closure_authority_v1(
                    closure_inputs,
                    thickness,
                    issuer_context,
                )
            }
        }
        .expect("complete multi-block closure authority");
        assert_eq!(
            parent.binding,
            direct_closure_binding_v1(&parent, domains.closure),
            "closure fingerprint changed at arity {block_count}",
        );
        if block_count > MULTI_BLOCK_MAX_BLOCKS_V1 {
            assert_ne!(
                parent.binding,
                direct_closure_binding_v1(&parent, b"closure_v1"),
                "exact closure domain collapsed into the generic domain at arity {block_count}",
            );
        }
        let proofs = prepared
            .iter()
            .zip(&scheduled)
            .map(|((_, geometry, audit, _, source), (schedule, closure))| {
                let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
                    geometry,
                    audit,
                    closure.fixed_face(),
                    schedule,
                    closure,
                    thickness,
                    32,
                )
                .expect("complete multi-block positive path");
                let layer =
                    certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                        geometry,
                        audit,
                        source,
                        schedule,
                        closure,
                        positive_continuous: &positive,
                        paper_thickness_mm: thickness,
                        tolerance: 1.0e-8,
                        limits: GeneralCellTransportLimitsV1 {
                            max_transitions: closure.leaves().len() + 1,
                            max_cells: 1_000_000,
                            max_layer_records: 1_000_000,
                            max_boundary_samples: 1_000_000,
                        },
                    })
                    .expect("complete multi-block layer transport");
                (positive, layer)
            })
            .collect::<Vec<_>>();
        let parent = issue_multi_block_positive_layer_authority_v1(
            parent,
            prepared
                .iter()
                .zip(proofs)
                .map(|((_, geometry, _, _, source), (positive, layer))| {
                    MultiBlockPositiveLayerInputV1 {
                        geometry,
                        source,
                        positive,
                        layer,
                    }
                })
                .collect(),
            layer_fingerprint,
        )
        .expect("complete multi-block positive layer authority");
        assert_eq!(
            parent.binding,
            direct_positive_layer_binding_v1(&parent, domains.positive),
            "positive-layer fingerprint changed at arity {block_count}",
        );
        if block_count > MULTI_BLOCK_MAX_BLOCKS_V1 {
            assert_ne!(
                parent.binding,
                direct_positive_layer_binding_v1(&parent, None),
                "exact positive-layer domain collapsed into the generic domain at arity {block_count}",
            );
        }
        let sources = prepared
            .iter()
            .map(|(_, _, _, _, source)| source)
            .collect::<Vec<&LayerOrderSnapshot>>();
        let target_angles = prepared
            .iter()
            .zip(&scheduled)
            .flat_map(|((_, _, _, _, _), (schedule, _))| {
                schedule
                    .evaluate(1.0)
                    .expect("target angles")
                    .as_slice()
                    .iter()
                    .map(|angle| (angle.edge(), angle.angle_degrees()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let block_faces = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| geometry.face_ids().to_vec())
            .collect::<Vec<_>>();
        let block_hinges = prepared
            .iter()
            .map(|(_, geometry, _, _, _)| {
                geometry
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut omitted_faces = block_faces.clone();
        let omitted_face = omitted_faces[0]
            .iter()
            .position(|face| {
                !omitted_faces[1..]
                    .iter()
                    .any(|candidate| candidate.contains(face))
            })
            .expect("unique omitted face");
        omitted_faces[0].remove(omitted_face);
        assert!(
            !completeness_report_v1(&live_geometry, &omitted_faces, &block_hinges)
                .expect("omitted face report")
                .exact_live_union_observed()
        );
        let mut extra_faces = block_faces.clone();
        extra_faces[0].push(FaceId::new());
        assert!(
            !completeness_report_v1(&live_geometry, &extra_faces, &block_hinges)
                .expect("extra face report")
                .exact_live_union_observed()
        );
        let mut duplicate_faces = block_faces.clone();
        let duplicate_face = duplicate_faces[0][0];
        duplicate_faces[0].push(duplicate_face);
        assert!(completeness_report_v1(&live_geometry, &duplicate_faces, &block_hinges).is_none());

        let mut omitted_hinges = block_hinges.clone();
        omitted_hinges[0].pop();
        assert!(
            !completeness_report_v1(&live_geometry, &block_faces, &omitted_hinges)
                .expect("omitted hinge report")
                .exact_live_union_observed()
        );
        let mut extra_hinges = block_hinges.clone();
        extra_hinges[0].push(EdgeId::new());
        assert!(
            !completeness_report_v1(&live_geometry, &block_faces, &extra_hinges)
                .expect("extra hinge report")
                .exact_live_union_observed()
        );
        let mut duplicate_hinges = block_hinges.clone();
        let duplicate_hinge = duplicate_hinges[1][0];
        duplicate_hinges[0].push(duplicate_hinge);
        assert!(
            !completeness_report_v1(&live_geometry, &block_faces, &duplicate_hinges)
                .expect("duplicate hinge report")
                .exact_live_union_observed()
        );

        let mut disconnected_faces = block_faces.clone();
        let mut adjacent = None;
        'adjacent: for first in 0..block_count {
            for second in first + 1..block_count {
                let shared = block_faces[first]
                    .iter()
                    .copied()
                    .filter(|face| block_faces[second].contains(face))
                    .collect::<Vec<_>>();
                if shared.len() == 1 {
                    adjacent = Some((first, second, shared[0]));
                    break 'adjacent;
                }
            }
        }
        let (first_adjacent, _, first_articulation) = adjacent.expect("adjacent blocks");
        disconnected_faces[first_adjacent].retain(|face| *face != first_articulation);
        assert!(
            !completeness_report_v1(&live_geometry, &disconnected_faces, &block_hinges)
                .expect("disconnected report")
                .exact_live_union_observed()
        );
        let mut cyclic_faces = block_faces.clone();
        let mut non_adjacent = None;
        'non_adjacent: for first in 0..block_count {
            for second in first + 1..block_count {
                if block_faces[first]
                    .iter()
                    .all(|face| !block_faces[second].contains(face))
                {
                    non_adjacent = Some((first, second));
                    break 'non_adjacent;
                }
            }
        }
        let (cycle_first, cycle_second) = non_adjacent.expect("non-adjacent blocks");
        let cycle_face = cyclic_faces[cycle_second]
            .iter()
            .copied()
            .find(|face| {
                !cyclic_faces
                    .iter()
                    .enumerate()
                    .any(|(index, candidate)| index != cycle_second && candidate.contains(face))
            })
            .expect("cycle face");
        cyclic_faces[cycle_first].push(cycle_face);
        assert!(
            !completeness_report_v1(&live_geometry, &cyclic_faces, &block_hinges)
                .expect("cycle report")
                .exact_live_union_observed()
        );

        let mut mismatched_faces = block_faces.clone();
        let first_unique = mismatched_faces[0]
            .iter()
            .position(|face| {
                !mismatched_faces[1..]
                    .iter()
                    .any(|candidate| candidate.contains(face))
            })
            .expect("first unique face");
        let second_unique = mismatched_faces[1]
            .iter()
            .position(|face| {
                !mismatched_faces
                    .iter()
                    .enumerate()
                    .any(|(index, candidate)| index != 1 && candidate.contains(face))
            })
            .expect("second unique face");
        let first = mismatched_faces[0][first_unique];
        mismatched_faces[0][first_unique] = mismatched_faces[1][second_unique];
        mismatched_faces[1][second_unique] = first;
        let mismatched_report =
            completeness_report_v1(&live_geometry, &mismatched_faces, &block_hinges)
                .expect("mismatched complete report");
        assert!(mismatched_report.exact_live_union_observed());
        assert_ne!(
            super::owned_multi_block_bindings_v1(&parent),
            mismatched_report.blocks
        );
        assert!(!super::complete_multi_block_report_matches_parent_v1(
            &live_geometry,
            &mismatched_report,
            &parent,
        ));

        let mut reversed_faces = block_faces.clone();
        let mut reversed_hinges = block_hinges.clone();
        reversed_faces.reverse();
        reversed_hinges.reverse();
        let report = completeness_report_v1(&live_geometry, &reversed_faces, &reversed_hinges)
            .expect("canonical complete report");
        assert!(report.exact_live_union_observed());
        let wrong_scopes = [
            super::MultiBlockAdmissionScopeV1::GenericSubmitted2To8,
            super::MultiBlockAdmissionScopeV1::ExactNineSubmittedSet,
            super::MultiBlockAdmissionScopeV1::ExactTenSubmittedSet,
        ]
        .into_iter()
        .filter(|scope| *scope != expected_scope)
        .collect::<Vec<_>>();
        assert_eq!(wrong_scopes.len(), 2);
        assert_eq!(report.scope, expected_scope);
        assert_eq!(parent.scope, expected_scope);
        for wrong_scope in &wrong_scopes {
            let mut wrong_scope_report = report.clone();
            wrong_scope_report.scope = *wrong_scope;
            assert!(!super::complete_multi_block_report_matches_parent_v1(
                &live_geometry,
                &wrong_scope_report,
                &parent,
            ));
        }
        assert!(!super::complete_multi_block_report_matches_parent_v1(
            &detached_live_geometry,
            &report,
            &parent,
        ));
        let mut authority = issue_complete_multi_block_positive_layer_authority_v1(
            &live_geometry,
            report,
            parent,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        )
        .expect("sealed complete multi-block authority");
        assert_eq!(
            authority.binding,
            direct_complete_binding_v1(&authority, domains.complete),
            "complete-live fingerprint changed at arity {block_count}",
        );
        if block_count > MULTI_BLOCK_MAX_BLOCKS_V1 {
            assert_ne!(
                authority.binding,
                direct_complete_binding_v1(&authority, None),
                "exact complete-live domain collapsed into the generic domain at arity {block_count}",
            );
        }
        assert_eq!(
            authority.model_id(),
            COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1
        );
        assert_eq!(authority.block_count_v1(), block_count);
        assert!(authority.exact_live_union_certified_v1());
        assert!(!authority.authorizes_project_mutation());
        assert!(!authority.authorizes_apply());
        assert!(!authority.authorizes_viewer());
        assert!(authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        assert_eq!(authority.scope, expected_scope);
        assert_eq!(authority.parent.scope, expected_scope);
        assert_eq!(authority.parent.parent.scope, expected_scope);
        for wrong_scope in wrong_scopes {
            authority.scope = wrong_scope;
            assert!(!authority.revalidates_v1(
                &live_geometry,
                &sources,
                thickness,
                issuer_context,
                layer_fingerprint,
                &target_angles,
            ));
            authority.scope = expected_scope;
            authority.parent.scope = wrong_scope;
            assert!(!authority.revalidates_v1(
                &live_geometry,
                &sources,
                thickness,
                issuer_context,
                layer_fingerprint,
                &target_angles,
            ));
            authority.parent.scope = expected_scope;
            authority.parent.parent.scope = wrong_scope;
            assert!(!authority.revalidates_v1(
                &live_geometry,
                &sources,
                thickness,
                issuer_context,
                layer_fingerprint,
                &target_angles,
            ));
            authority.parent.parent.scope = expected_scope;
        }
        assert!(authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        assert!(!authority.revalidates_v1(
            &detached_live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        let mut reordered_sources = sources.clone();
        reordered_sources.swap(0, 1);
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &reordered_sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness + 0.1,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            [0x50; 32],
            layer_fingerprint,
            &target_angles,
        ));
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            [0x53; 32],
            &target_angles,
        ));
        let mut wrong_target = target_angles.clone();
        wrong_target[0].1 = f64::from_bits(wrong_target[0].1.to_bits() ^ 1);
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &wrong_target,
        ));
        let original_faces = authority.parent.parent.blocks[0].faces.clone();
        authority.parent.parent.blocks[0].faces[0] = FaceId::new();
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        authority.parent.parent.blocks[0].faces = original_faces;
        let original_binding = authority.binding;
        authority.binding[0] ^= 1;
        assert!(!authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
        authority.binding = original_binding;
        assert!(authority.revalidates_v1(
            &live_geometry,
            &sources,
            thickness,
            issuer_context,
            layer_fingerprint,
            &target_angles,
        ));
    }

    #[test]
    fn representative_generic_complete_authorities_preserve_legacy_fingerprints() {
        for block_count in [3_usize, 4, 5, MULTI_BLOCK_MAX_BLOCKS_V1] {
            assert_complete_live_multi_block_authority_v1(block_count);
        }
    }

    #[test]
    fn exact_nine_complete_live_authority_is_explicitly_scoped_and_non_authorizing() {
        assert_complete_live_multi_block_authority_v1(9);
    }

    #[test]
    fn exact_ten_complete_live_authority_is_explicitly_scoped_and_non_authorizing() {
        assert_complete_live_multi_block_authority_v1(10);
    }
}
