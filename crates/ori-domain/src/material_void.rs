use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CreasePattern, EdgeId, EdgeKind, ProjectId, VertexId};

pub const MATERIAL_VOID_EVIDENCE_VERSION_V1: u8 = 1;
pub const MATERIAL_VOID_EVIDENCE_MODEL_ID_V1: &str = "passive_material_void_evidence_v1";
pub const MAX_MATERIAL_VOID_REGIONS_V1: usize = 64;
pub const MAX_MATERIAL_VOID_LOOP_EDGES_V1: usize = 16_384;
pub const MAX_MATERIAL_VOID_TOTAL_LOOP_EDGES_V1: usize = 16_384;
pub const MAX_MATERIAL_VOID_REMOVED_COMPONENTS_V1: usize = 64;
pub const MAX_MATERIAL_VOID_PATTERN_EDGES_V1: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialVoidRegionEvidenceV1 {
    pub region_id_sha256: [u8; 32],
    pub removal_plan_sha256: [u8; 32],
    pub removed_component_keys: Vec<[u8; 32]>,
    pub boundary_edge_loop: Vec<EdgeId>,
}

/// Passive evidence that one or more closed `Cut` loops bound material voids.
///
/// The referenced pattern remains unchanged: loop edges stay `EdgeKind::Cut`,
/// not `EdgeKind::Boundary`. Only a future extension-aware topology consumer
/// may interpret the retained side as a one-sided material boundary. This
/// record grants no editor, persistence, export, or simulation authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialVoidEvidenceDocumentV1 {
    pub version: u8,
    pub source_project_id: Option<ProjectId>,
    pub source_revision: u64,
    /// Fold-model fingerprint before removed-only interior geometry was deleted.
    pub source_fold_model_fingerprint: String,
    /// Fold-model fingerprint of the persisted post-removal paper and pattern.
    pub post_fold_model_fingerprint: String,
    pub regions: Vec<MaterialVoidRegionEvidenceV1>,
}

impl Default for MaterialVoidEvidenceDocumentV1 {
    fn default() -> Self {
        Self {
            version: MATERIAL_VOID_EVIDENCE_VERSION_V1,
            source_project_id: None,
            source_revision: 0,
            source_fold_model_fingerprint: String::new(),
            post_fold_model_fingerprint: String::new(),
            regions: Vec::new(),
        }
    }
}

impl MaterialVoidEvidenceDocumentV1 {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        MATERIAL_VOID_EVIDENCE_MODEL_ID_V1
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialVoidEvidenceErrorV1 {
    UnsupportedVersion,
    InvalidSourceBinding,
    ResourceLimit,
    NonCanonicalRegionOrder,
    InvalidRegionIdentity,
    InvalidRemovedComponents,
    InvalidBoundaryLoop,
    UnknownBoundaryEdge,
    NonCutBoundaryEdge,
    DuplicateCrossReference,
}

#[must_use]
pub fn material_void_region_id_sha256_v1(
    removal_plan_sha256: [u8; 32],
    removed_component_keys: &[[u8; 32]],
    boundary_edge_loop: &[EdgeId],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_MATERIAL_VOID_REGION_EVIDENCE_V1");
    hash.update(removal_plan_sha256);
    hash.update((removed_component_keys.len() as u64).to_be_bytes());
    for key in removed_component_keys {
        hash.update(key);
    }
    hash.update((boundary_edge_loop.len() as u64).to_be_bytes());
    for edge in boundary_edge_loop {
        hash.update(edge.canonical_bytes());
    }
    hash.finalize().into()
}

pub fn validate_material_void_evidence_v1(
    evidence: &MaterialVoidEvidenceDocumentV1,
    pattern: &CreasePattern,
) -> Result<(), MaterialVoidEvidenceErrorV1> {
    if evidence.version != MATERIAL_VOID_EVIDENCE_VERSION_V1 {
        return Err(MaterialVoidEvidenceErrorV1::UnsupportedVersion);
    }
    if evidence.regions.is_empty() {
        return if evidence.is_default() {
            Ok(())
        } else {
            Err(MaterialVoidEvidenceErrorV1::InvalidSourceBinding)
        };
    }
    if evidence.source_project_id.is_none()
        || (!evidence.regions.is_empty()
            && (!valid_sha256_hex(&evidence.source_fold_model_fingerprint)
                || !valid_sha256_hex(&evidence.post_fold_model_fingerprint)))
    {
        return Err(MaterialVoidEvidenceErrorV1::InvalidSourceBinding);
    }
    if evidence.regions.len() > MAX_MATERIAL_VOID_REGIONS_V1
        || pattern.edges.len() > MAX_MATERIAL_VOID_PATTERN_EDGES_V1
    {
        return Err(MaterialVoidEvidenceErrorV1::ResourceLimit);
    }
    if evidence
        .regions
        .windows(2)
        .any(|pair| pair[0].region_id_sha256.as_slice() >= pair[1].region_id_sha256.as_slice())
    {
        return Err(MaterialVoidEvidenceErrorV1::NonCanonicalRegionOrder);
    }
    let mut edge_by_id = HashMap::new();
    edge_by_id
        .try_reserve(pattern.edges.len())
        .map_err(|_| MaterialVoidEvidenceErrorV1::ResourceLimit)?;
    for edge in &pattern.edges {
        if edge_by_id.insert(edge.id, edge).is_some() {
            return Err(MaterialVoidEvidenceErrorV1::DuplicateCrossReference);
        }
    }
    let mut referenced_edges = HashSet::new();
    let mut removed_components = HashSet::new();
    let total_loop_edges = evidence
        .regions
        .iter()
        .try_fold(0_usize, |sum, region| {
            sum.checked_add(region.boundary_edge_loop.len())
        })
        .ok_or(MaterialVoidEvidenceErrorV1::ResourceLimit)?;
    if total_loop_edges > MAX_MATERIAL_VOID_TOTAL_LOOP_EDGES_V1 {
        return Err(MaterialVoidEvidenceErrorV1::ResourceLimit);
    }
    referenced_edges
        .try_reserve(total_loop_edges)
        .map_err(|_| MaterialVoidEvidenceErrorV1::ResourceLimit)?;
    removed_components
        .try_reserve(
            evidence
                .regions
                .iter()
                .try_fold(0_usize, |sum, region| {
                    sum.checked_add(region.removed_component_keys.len())
                })
                .ok_or(MaterialVoidEvidenceErrorV1::ResourceLimit)?,
        )
        .map_err(|_| MaterialVoidEvidenceErrorV1::ResourceLimit)?;
    for region in &evidence.regions {
        if region.removal_plan_sha256 == [0; 32]
            || region.removed_component_keys.is_empty()
            || region.removed_component_keys.len() > MAX_MATERIAL_VOID_REMOVED_COMPONENTS_V1
            || region.boundary_edge_loop.len() < 3
            || region.boundary_edge_loop.len() > MAX_MATERIAL_VOID_LOOP_EDGES_V1
        {
            return Err(MaterialVoidEvidenceErrorV1::ResourceLimit);
        }
        if region.removed_component_keys.contains(&[0; 32])
            || region
                .removed_component_keys
                .windows(2)
                .any(|pair| pair[0].as_slice() >= pair[1].as_slice())
        {
            return Err(MaterialVoidEvidenceErrorV1::InvalidRemovedComponents);
        }
        if region
            .removed_component_keys
            .iter()
            .any(|key| !removed_components.insert(*key))
        {
            return Err(MaterialVoidEvidenceErrorV1::DuplicateCrossReference);
        }
        if !is_canonical_loop(&region.boundary_edge_loop) {
            return Err(MaterialVoidEvidenceErrorV1::InvalidBoundaryLoop);
        }
        let mut endpoints = Vec::new();
        endpoints
            .try_reserve_exact(region.boundary_edge_loop.len())
            .map_err(|_| MaterialVoidEvidenceErrorV1::ResourceLimit)?;
        for edge_id in &region.boundary_edge_loop {
            if !referenced_edges.insert(*edge_id) {
                return Err(MaterialVoidEvidenceErrorV1::DuplicateCrossReference);
            }
            let edge = edge_by_id
                .get(edge_id)
                .ok_or(MaterialVoidEvidenceErrorV1::UnknownBoundaryEdge)?;
            if edge.kind != EdgeKind::Cut {
                return Err(MaterialVoidEvidenceErrorV1::NonCutBoundaryEdge);
            }
            if edge.start == edge.end {
                return Err(MaterialVoidEvidenceErrorV1::InvalidBoundaryLoop);
            }
            endpoints.push((edge.start, edge.end));
        }
        validate_connected_cycle(&endpoints)?;
        if region.region_id_sha256
            != material_void_region_id_sha256_v1(
                region.removal_plan_sha256,
                &region.removed_component_keys,
                &region.boundary_edge_loop,
            )
        {
            return Err(MaterialVoidEvidenceErrorV1::InvalidRegionIdentity);
        }
    }
    Ok(())
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_canonical_loop(edges: &[EdgeId]) -> bool {
    let Some(first) = edges.first().map(EdgeId::canonical_bytes) else {
        return false;
    };
    edges
        .iter()
        .skip(1)
        .all(|edge| first < edge.canonical_bytes())
        && edges[1].canonical_bytes() < edges[edges.len() - 1].canonical_bytes()
}

fn validate_connected_cycle(
    endpoints: &[(VertexId, VertexId)],
) -> Result<(), MaterialVoidEvidenceErrorV1> {
    let mut degree = HashMap::<VertexId, usize>::new();
    degree
        .try_reserve(endpoints.len())
        .map_err(|_| MaterialVoidEvidenceErrorV1::ResourceLimit)?;
    for (start, end) in endpoints {
        *degree.entry(*start).or_default() += 1;
        *degree.entry(*end).or_default() += 1;
    }
    if degree.len() != endpoints.len() || degree.values().any(|degree| *degree != 2) {
        return Err(MaterialVoidEvidenceErrorV1::InvalidBoundaryLoop);
    }
    for index in 0..endpoints.len() {
        let first = endpoints[index];
        let second = endpoints[(index + 1) % endpoints.len()];
        let shared = [first.0, first.1]
            .into_iter()
            .filter(|vertex| *vertex == second.0 || *vertex == second.1)
            .count();
        if shared != 1 {
            return Err(MaterialVoidEvidenceErrorV1::InvalidBoundaryLoop);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Point2, Vertex};

    fn valid_fixture() -> (CreasePattern, MaterialVoidEvidenceDocumentV1) {
        let vertices = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let mut edge_ids = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
        edge_ids.sort_unstable_by_key(EdgeId::canonical_bytes);
        let pattern = CreasePattern {
            vertices: vertices
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 0.0),
                    Point2::new(1.0, 1.0),
                    Point2::new(0.0, 1.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: edge_ids
                .into_iter()
                .enumerate()
                .map(|(index, id)| Edge {
                    id,
                    start: vertices[index],
                    end: vertices[(index + 1) % vertices.len()],
                    kind: EdgeKind::Cut,
                })
                .collect(),
        };
        let removal_plan_sha256 = [3; 32];
        let removed_component_keys = vec![[4; 32]];
        let boundary_edge_loop = edge_ids.to_vec();
        let region_id_sha256 = material_void_region_id_sha256_v1(
            removal_plan_sha256,
            &removed_component_keys,
            &boundary_edge_loop,
        );
        (
            pattern,
            MaterialVoidEvidenceDocumentV1 {
                version: 1,
                source_project_id: Some(ProjectId::new()),
                source_revision: 7,
                source_fold_model_fingerprint: "a".repeat(64),
                post_fold_model_fingerprint: "b".repeat(64),
                regions: vec![MaterialVoidRegionEvidenceV1 {
                    region_id_sha256,
                    removal_plan_sha256,
                    removed_component_keys,
                    boundary_edge_loop,
                }],
            },
        )
    }

    #[test]
    fn passive_void_evidence_is_strictly_bound_and_nonauthoritative() {
        let (pattern, evidence) = valid_fixture();
        validate_material_void_evidence_v1(&evidence, &pattern).unwrap();
        assert!(!evidence.authorizes_project_mutation());
        assert!(!evidence.authorizes_material_removal());
        assert!(!evidence.authorizes_persistence());
        assert!(!evidence.authorizes_simulation_admission());

        let mut tampered = evidence.clone();
        tampered.regions[0].region_id_sha256[0] ^= 1;
        assert_eq!(
            validate_material_void_evidence_v1(&tampered, &pattern),
            Err(MaterialVoidEvidenceErrorV1::InvalidRegionIdentity)
        );
        let mut reversed = evidence.clone();
        reversed.regions[0].boundary_edge_loop.reverse();
        assert_eq!(
            validate_material_void_evidence_v1(&reversed, &pattern),
            Err(MaterialVoidEvidenceErrorV1::InvalidBoundaryLoop)
        );
    }
}
