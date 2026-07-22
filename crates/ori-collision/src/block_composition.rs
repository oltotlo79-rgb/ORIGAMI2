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
    let mut has_articulation = false;
    for first in 0..canonical.len() {
        for second in first + 1..canonical.len() {
            let shared = canonical[first]
                .faces
                .iter()
                .filter(|face| {
                    canonical[second]
                        .faces
                        .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
                        .is_ok()
                })
                .count();
            if shared > 1 {
                return None;
            }
            has_articulation |= shared == 1;
        }
    }
    if !has_articulation {
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
