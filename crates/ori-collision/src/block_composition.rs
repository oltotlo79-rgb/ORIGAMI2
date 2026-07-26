use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId};
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};

use crate::{GeneralMultiFaceCellTransportProofV1, PositiveThicknessContinuousCertificateV1};

pub const BLOCK_COMPOSED_PATH_MODEL_ID_V1: &str = "block_composed_path_authority_v1";
pub const BLOCK_COMPOSITION_LIMIT_V1: usize = 32;
pub const BLOCKWISE_CLOSURE_MODEL_ID_V1: &str = "blockwise_interval_closure_authority_v1";
pub const BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1: &str = "blockwise_positive_layer_authority_v1";
pub const BLOCKWISE_POSITIVE_LAYER_ARITY_V1: usize = 2;
pub const MULTI_BLOCK_MIN_BLOCKS_V1: usize = 2;
pub const MULTI_BLOCK_MAX_BLOCKS_V1: usize = 8;
pub const MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1: &str =
    "bounded_multi_block_positive_layer_authority_v1";
pub const COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1: &str =
    "complete_live_multi_block_positive_layer_authority_v1";
pub const BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1: usize = 4_096;

pub struct BlockUnionCompletenessInputV1<'a> {
    pub faces: &'a [FaceId],
    pub hinges: &'a [EdgeId],
}

#[derive(Debug, Clone)]
pub struct BlockUnionCompletenessGapReportV1 {
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

#[must_use]
pub fn diagnose_block_union_completeness_v1(
    geometry: &MaterialHingeGraphGeometry,
    blocks: &[BlockUnionCompletenessInputV1<'_>],
) -> Option<BlockUnionCompletenessGapReportV1> {
    let live_item_count = geometry
        .face_ids()
        .len()
        .checked_add(geometry.hinges().len())?;
    if !(2..=MULTI_BLOCK_MAX_BLOCKS_V1).contains(&blocks.len())
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
        && block_intersection_is_tree_v1(&canonical_blocks);
    Some(BlockUnionCompletenessGapReportV1 {
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
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
}

/// Sealed authority for one submitted 2..=8 block tree.
///
/// This is deliberately not whole-graph or project-mutation authority. A
/// future production adapter must separately bind the canonical union of all
/// submitted hinges to the complete live graph before relying on it.
pub struct MultiBlockClosureAuthorityV1 {
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
        self.issuer.same_instance(live_geometry)
            && self.live_faces == canonical_faces_v1(live_geometry)
            && self.live_hinges == canonical_hinges_v1(live_geometry)
            && complete_block_union_matches_live_v1(
                &self.blocks,
                &self.live_faces,
                &self.live_hinges,
            )
            && owned_multi_block_bindings_v1(&self.parent) == self.blocks
            && self.parent.revalidates_v1(
                sources,
                thickness,
                issuer_context,
                articulation_layer_fingerprint,
            )
            && self.parent.target_angles_match_v1(target_angles)
            && complete_multi_block_positive_layer_binding_v1(
                self.parent.binding,
                &self.live_faces,
                &self.live_hinges,
                &self.blocks,
            ) == self.binding
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
        let mut expected = Vec::new();
        for block in &self.parent.blocks {
            let Some(endpoint) = block.schedule.evaluate(1.0) else {
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
        sources: &[&LayerOrderSnapshot],
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
    ) -> bool {
        if sources.len() != self.parent.blocks.len()
            || thickness.to_bits() != self.parent.thickness_bits
            || issuer_context != self.parent.issuer_context
            || issuer_context == [0; 32]
            || articulation_layer_fingerprint != self.articulation_layer_fingerprint
            || articulation_layer_fingerprint == [0; 32]
        {
            return false;
        }
        for (index, source) in sources.iter().enumerate() {
            let block = &self.parent.blocks[index];
            let fixed_face = block.closure.fixed_face();
            if !self.positive[index].is_for(
                &block.geometry,
                fixed_face,
                &block.schedule,
                &block.closure,
                thickness,
            ) || !self.layer[index].is_for(
                &block.geometry,
                source,
                &block.schedule,
                &block.closure,
                thickness,
            ) {
                return false;
            }
        }
        multi_block_positive_layer_binding_v1(
            self.parent.binding,
            &self.layer,
            articulation_layer_fingerprint,
        ) == self.binding
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
                    .map(|(geometry, schedule, closure)| (geometry, schedule, closure));
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
        for (_, schedule, _) in &self.parent.blocks {
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
            let (geometry, schedule, closure) = &self.parent.blocks[index];
            if !self.positive[index].is_for(geometry, articulation, schedule, closure, thickness)
                || !self.layer[index].is_for(geometry, source, schedule, closure, thickness)
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
        let (geometry, schedule, closure) = &parent.blocks[index];
        if !input
            .positive
            .is_for(geometry, articulation, schedule, closure, thickness)
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
                schedule.certificate_binding_fingerprint_v1(),
                closure.partition_binding_fingerprint_v1(),
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

fn block_intersection_is_tree_v1(blocks: &[CanonicalBlockBindingV1]) -> bool {
    if blocks.len() < 2 {
        return false;
    }
    let mut adjacency = vec![Vec::new(); blocks.len()];
    let mut edge_count = 0usize;
    for first in 0..blocks.len() {
        for second in first + 1..blocks.len() {
            let shared = blocks[first]
                .faces
                .iter()
                .filter(|face| {
                    blocks[second]
                        .faces
                        .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
                        .is_ok()
                })
                .count();
            if shared > 1 {
                return false;
            }
            if shared == 1 {
                adjacency[first].push(second);
                adjacency[second].push(first);
                edge_count += 1;
            }
        }
    }
    if edge_count != blocks.len() - 1 {
        return false;
    }
    let mut visited = vec![false; blocks.len()];
    let mut pending = vec![0usize];
    visited[0] = true;
    while let Some(block) = pending.pop() {
        for &neighbor in &adjacency[block] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                pending.push(neighbor);
            }
        }
    }
    visited.into_iter().all(|seen| seen)
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
    if !multi_block_count_supported_v1(inputs.len())
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
    if !block_intersection_is_tree_v1(&canonical) {
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
    hash.update(b"closure_v1");
    hash.update(thickness.to_bits().to_le_bytes());
    hash.update(issuer_context);
    for block in &blocks {
        hash.update(block.schedule.graph_binding_fingerprint_v1());
        hash.update(block.schedule.certificate_binding_fingerprint_v1());
        hash.update(block.closure.partition_binding_fingerprint_v1());
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    Some(MultiBlockClosureAuthorityV1 {
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
    if inputs.len() != parent.blocks.len() || articulation_layer_fingerprint == [0; 32] {
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
        parent.binding,
        &layer,
        articulation_layer_fingerprint,
    );
    Some(MultiBlockPositiveLayerAuthorityV1 {
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
    let binding = complete_multi_block_positive_layer_binding_v1(
        parent.binding,
        &report.live_faces,
        &report.live_hinges,
        &report.blocks,
    );
    let authority = CompleteMultiBlockPositiveLayerAuthorityV1 {
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
    report.is_for(live_geometry)
        && report.complete
        && complete_block_union_matches_live_v1(
            &report.blocks,
            &report.live_faces,
            &report.live_hinges,
        )
        && owned_multi_block_bindings_v1(parent) == report.blocks
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

fn complete_block_union_matches_live_v1(
    blocks: &[CanonicalBlockBindingV1],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
) -> bool {
    if !multi_block_count_supported_v1(blocks.len())
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
        || !block_intersection_is_tree_v1(blocks)
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

fn complete_multi_block_positive_layer_binding_v1(
    parent_binding: [u8; 32],
    live_faces: &[FaceId],
    live_hinges: &[EdgeId],
    blocks: &[CanonicalBlockBindingV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
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

fn multi_block_positive_layer_binding_v1(
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
        source: &LayerOrderSnapshot,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        thickness: f64,
        articulation_pose_fingerprint: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
    ) -> bool {
        self.positive
            .is_for(geometry, fixed_face, schedule, closure, thickness)
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
    hash.update(schedule.certificate_binding_fingerprint_v1());
    hash.update(closure.partition_binding_fingerprint_v1());
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
        || !positive.is_for(geometry, fixed_face, schedule, closure, thickness)
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
    if !block_intersection_is_tree_v1(&canonical) {
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

#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../test-support/miura_cactus.rs"]
mod miura_cactus_test_support;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashSet};

    use super::{
        COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1, CanonicalBlockBindingV1,
        MULTI_BLOCK_MAX_BLOCKS_V1, MULTI_BLOCK_MIN_BLOCKS_V1, MultiBlockClosureInputV1,
        MultiBlockPositiveLayerInputV1, block_intersection_is_tree_v1,
        issue_complete_multi_block_positive_layer_authority_v1,
        issue_multi_block_closure_authority_v1, issue_multi_block_positive_layer_authority_v1,
        multi_block_count_supported_v1,
    };
    use crate::{
        GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
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
        CanonicalCycleScheduleV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
        HalfAngleRationalEntryInputV1, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
        RationalCoefficientV1, TreeKinematicsLimits,
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
        assert!((3..=MULTI_BLOCK_MAX_BLOCKS_V1).contains(&block_count));
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
                                    numerator: i64::from(moves),
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
        super::diagnose_block_union_completeness_v1(geometry, &inputs)
    }

    #[test]
    fn block_intersection_requires_one_connected_articulation_tree() {
        let [a, b, c, d] = std::array::from_fn(|_| FaceId::new());
        assert!(block_intersection_is_tree_v1(&[
            block(&[a, b]),
            block(&[b, c]),
            block(&[c, d]),
        ]));
    }

    #[test]
    fn block_intersection_rejects_an_isolated_block() {
        let [a, b, c, d] = std::array::from_fn(|_| FaceId::new());
        assert!(!block_intersection_is_tree_v1(&[
            block(&[a, b]),
            block(&[b, c]),
            block(&[d]),
        ]));
    }

    #[test]
    fn block_intersection_rejects_an_articulation_cycle() {
        let [a, b, c] = std::array::from_fn(|_| FaceId::new());
        assert!(!block_intersection_is_tree_v1(&[
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
                                    numerator: i64::from(moves),
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

    fn assert_complete_live_multi_block_authority_v1(block_count: usize) {
        let (prepared, scheduled, live_geometry, detached_live_geometry) =
            prepare_complete_chain_v1(block_count);
        assert_eq!(live_geometry, detached_live_geometry);
        assert!(!live_geometry.same_instance(&detached_live_geometry));
        let thickness = 0.1;
        let issuer_context = [0x51; 32];
        let layer_fingerprint = [0x52; 32];
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
        .expect("complete multi-block closure authority");
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
    fn complete_live_three_four_and_eight_block_authorities_are_sealed_and_non_authorizing() {
        for block_count in [3_usize, 4, 8] {
            assert_complete_live_multi_block_authority_v1(block_count);
        }
    }
}
