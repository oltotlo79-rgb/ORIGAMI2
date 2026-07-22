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

/// Owns the already-issued whole-graph proofs and binds them to one canonical
/// edge partition. Callers can neither manufacture a partial block proof nor
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
    use super::{
        CanonicalBlockBindingV1, MULTI_BLOCK_MAX_BLOCKS_V1, MULTI_BLOCK_MIN_BLOCKS_V1,
        MultiBlockClosureInputV1, MultiBlockPositiveLayerInputV1, block_intersection_is_tree_v1,
        issue_multi_block_closure_authority_v1, issue_multi_block_positive_layer_authority_v1,
        multi_block_count_supported_v1,
    };
    use crate::{
        GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
        certify_canonical_positive_thickness_cycle_schedule_path_v1,
        certify_general_multi_face_cell_transport_v1,
    };
    use ori_core::{analyze_global_flat_foldability, analyze_local_flat_foldability};
    use ori_domain::{EdgeId, FaceId, ProjectId};
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
}
