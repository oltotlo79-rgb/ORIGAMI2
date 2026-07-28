use sha2::{Digest, Sha256};

use super::{
    MATERIAL_RELIEF_DOCUMENT_VERSION_V1, MaterialReliefDocumentV1, MaterialReliefLineageId,
    MaterialReliefRegionV1, material_relief_state_sha256_v1, material_relief_substrate_sha256_v1,
};
use crate::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};

pub(super) fn project_id() -> ProjectId {
    ProjectId::schema_namespace([
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x47, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        0x01,
    ])
}

fn square(name: &[u8], x_offset: f64) -> (Vec<Vertex>, Vec<Edge>, Vec<EdgeId>, [u8; 32]) {
    let namespace = project_id();
    let vertices = (0_u8..4)
        .map(|index| Vertex {
            id: VertexId::derive_v5(namespace, &[name, b"-vertex-", &[index]].concat()),
            position: Point2::new(
                x_offset + if index == 1 || index == 2 { 1.0 } else { 0.0 },
                if index >= 2 { 1.0 } else { 0.0 },
            ),
        })
        .collect::<Vec<_>>();
    let mut edge_ids = (0_u8..4)
        .map(|index| EdgeId::derive_v5(namespace, &[name, b"-edge-", &[index]].concat()))
        .collect::<Vec<_>>();
    edge_ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    let edges = edge_ids
        .iter()
        .enumerate()
        .map(|(index, id)| Edge {
            id: *id,
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Cut,
        })
        .collect::<Vec<_>>();
    let requested_component_key: [u8; 32] = Sha256::digest(name).into();
    (vertices, edges, edge_ids, requested_component_key)
}

pub(super) fn valid_fixture(
    region_count: usize,
) -> (CreasePattern, Paper, MaterialReliefDocumentV1) {
    assert!((1..=2).contains(&region_count));
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut drafts = Vec::new();
    for (name, x_offset) in [(b"first".as_slice(), 0.0), (b"second".as_slice(), 3.0)]
        .into_iter()
        .take(region_count)
    {
        let (square_vertices, square_edges, loop_edges, requested_component_key) =
            square(name, x_offset);
        vertices.extend(square_vertices);
        edges.extend(square_edges);
        drafts.push((requested_component_key, loop_edges));
    }
    drafts.sort_by_key(|(requested_component_key, _)| *requested_component_key);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: pattern
            .vertices
            .iter()
            .take(4)
            .map(|vertex| vertex.id)
            .collect(),
        cutting_allowed: true,
        ..Paper::default()
    };
    let substrate_fingerprint_sha256 =
        material_relief_substrate_sha256_v1(&pattern, &paper).unwrap();
    let regions = drafts
        .into_iter()
        .enumerate()
        .map(|(index, (requested_component_key, boundary_edge_loop))| {
            let mut removed_component_keys = vec![requested_component_key];
            let descendant: [u8; 32] =
                Sha256::digest([b"descendant-".as_slice(), &[index as u8]].concat()).into();
            removed_component_keys.push(descendant);
            removed_component_keys.sort_unstable();
            MaterialReliefRegionV1 {
                lineage_id: MaterialReliefLineageId::derive_v5(
                    project_id(),
                    substrate_fingerprint_sha256,
                    requested_component_key,
                ),
                requested_component_key,
                removed_component_keys,
                boundary_edge_loop,
            }
        })
        .collect::<Vec<_>>();
    let state_sha256 =
        material_relief_state_sha256_v1(project_id(), substrate_fingerprint_sha256, &regions)
            .unwrap();
    (
        pattern,
        paper,
        MaterialReliefDocumentV1 {
            version: MATERIAL_RELIEF_DOCUMENT_VERSION_V1,
            source_project_id: Some(project_id()),
            substrate_fingerprint_sha256,
            state_sha256,
            regions,
        },
    )
}
