use ori_domain::{CreasePattern, FaceId};
use ori_foldability::{LayerOrderDerivation, LayerOrderSnapshot};
use ori_topology::TopologySnapshot;
use std::collections::HashMap;

pub const MAX_CERTIFIED_FLAT_SURFACE_VERTICES_V1: usize = 256;
#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedFlatSurfaceFaceV1 {
    pub face: FaceId,
    pub boundary: Vec<[f64; 3]>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedFlatSurfaceV1 {
    pub source_revision: u64,
    pub faces: Vec<CertifiedFlatSurfaceFaceV1>,
}

pub fn extract_certified_flat_surface_v1(
    pattern: &CreasePattern,
    topology: &TopologySnapshot,
    certificate: &LayerOrderSnapshot,
) -> Option<CertifiedFlatSurfaceV1> {
    if topology.source_revision != certificate.provenance.source.source_revision
        || !matches!(
            certificate.provenance.derivation,
            LayerOrderDerivation::SingleFace { .. } | LayerOrderDerivation::SingleHinge { .. }
        )
        || certificate.folded_faces.len() != topology.faces.len()
    {
        return None;
    }
    let positions = pattern
        .vertices
        .iter()
        .map(|v| (v.id, v.position))
        .collect::<HashMap<_, _>>();
    let mut count = 0usize;
    let mut faces = Vec::with_capacity(topology.faces.len());
    for folded in &certificate.folded_faces {
        let source = topology
            .faces
            .iter()
            .find(|face| face.id == folded.face.face_id)?;
        count = count.checked_add(source.outer.half_edges.len())?;
        if count > MAX_CERTIFIED_FLAT_SURFACE_VERTICES_V1 || !source.holes.is_empty() {
            return None;
        }
        let t = &folded.source_to_flat;
        let c = [
            t.m00.to_f64()?,
            t.m01.to_f64()?,
            t.m10.to_f64()?,
            t.m11.to_f64()?,
            t.tx.to_f64()?,
            t.ty.to_f64()?,
        ];
        let boundary = source
            .outer
            .half_edges
            .iter()
            .map(|edge| {
                let p = positions.get(&edge.origin)?;
                Some([
                    c[0] * p.x + c[1] * p.y + c[4],
                    c[2] * p.x + c[3] * p.y + c[5],
                    0.0,
                ])
            })
            .collect::<Option<Vec<_>>>()?;
        if boundary.len() < 3 || boundary.iter().flatten().any(|v| !v.is_finite()) {
            return None;
        }
        faces.push(CertifiedFlatSurfaceFaceV1 {
            face: source.id,
            boundary,
        });
    }
    Some(CertifiedFlatSurfaceV1 {
        source_revision: topology.source_revision,
        faces,
    })
}
