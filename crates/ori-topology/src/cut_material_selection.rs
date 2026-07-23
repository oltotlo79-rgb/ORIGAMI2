//! Read-only material-component classification for closed cuts.
//!
//! The project/editor/history schemas have no persistent component keep/remove
//! selection. This module therefore exposes observations only and deliberately
//! provides no transaction or mutation conversion.

use std::collections::{HashMap, HashSet};

use ori_domain::FaceId;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClosedCutLoopDiagnosticLimitsV1, ClosedCutTopologySnapshotErrorV1, EdgeIncidence,
    FaceExtractionInput, MaterialComponentKey, diagnose_closed_cut_topology_snapshot_v1,
};

pub const CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1: &str =
    "cut_material_component_selection_diagnostic_v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CutMaterialComponentSelectionErrorV1 {
    #[error("closed-cut topology prerequisite failed: {0}")]
    Topology(#[from] ClosedCutTopologySnapshotErrorV1),
    #[error("material-component selection classification failed closed")]
    InvalidClassification,
}

/// Canonical, non-authoritative classification of one material component.
///
/// `owns_original_boundary` is a factual topology observation. Such a component
/// must never be removed. A component without the boundary is only a possible
/// removal candidate; neither value grants deletion authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutMaterialComponentSelectionV1 {
    pub component: MaterialComponentKey,
    pub faces: Vec<FaceId>,
    pub owns_original_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutMaterialComponentSelectionDiagnosticV1 {
    selections: Vec<CutMaterialComponentSelectionV1>,
    fingerprint: [u8; 32],
    limits: ClosedCutLoopDiagnosticLimitsV1,
}

impl CutMaterialComponentSelectionDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1
    }

    #[must_use]
    pub fn selections(&self) -> &[CutMaterialComponentSelectionV1] {
        &self.selections
    }

    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
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
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub fn is_for(&self, input: FaceExtractionInput<'_>) -> bool {
        diagnose_cut_material_component_selection_v1(input, self.limits)
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

pub fn diagnose_cut_material_component_selection_v1(
    input: FaceExtractionInput<'_>,
    limits: ClosedCutLoopDiagnosticLimitsV1,
) -> Result<CutMaterialComponentSelectionDiagnosticV1, CutMaterialComponentSelectionErrorV1> {
    let topology = diagnose_closed_cut_topology_snapshot_v1(input, limits)?;
    let snapshot = topology.snapshot();
    let paper_boundary_vertices = input
        .paper
        .boundary_vertices
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if input.pattern.edges.iter().any(|edge| {
        edge.kind == ori_domain::EdgeKind::Cut
            && (paper_boundary_vertices.contains(&edge.start)
                || paper_boundary_vertices.contains(&edge.end))
    }) {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }
    let face_ids = snapshot
        .faces
        .iter()
        .map(|face| face.id)
        .collect::<HashSet<_>>();
    let mut component_by_face = HashMap::with_capacity(face_ids.len());
    let mut component_keys = HashSet::with_capacity(snapshot.material_components.len());
    for component in &snapshot.material_components {
        if component.sheet_origin != input.identity_namespace
            || !component_keys.insert(component.key)
            || component.faces.is_empty()
        {
            return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
        }
        for face in &component.faces {
            if !face_ids.contains(face) || component_by_face.insert(*face, component.key).is_some()
            {
                return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
            }
        }
    }
    if component_by_face.len() != face_ids.len() {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let boundary_edges = input
        .pattern
        .edges
        .iter()
        .filter(|edge| edge.kind == ori_domain::EdgeKind::Boundary)
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    let mut boundary_components = HashSet::new();
    let mut observed_boundary_edges = HashSet::new();
    for (edge, incidence) in &snapshot.edge_incidence {
        if let EdgeIncidence::Boundary { material } = incidence {
            if !boundary_edges.contains(edge)
                || !observed_boundary_edges.insert(*edge)
                || !face_ids.contains(material)
            {
                return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
            }
            boundary_components.insert(component_by_face[material]);
        }
    }
    if observed_boundary_edges != boundary_edges || boundary_components.len() != 1 {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let mut selections = snapshot
        .material_components
        .iter()
        .map(|component| {
            let mut faces = component.faces.clone();
            faces.sort_unstable_by_key(FaceId::canonical_bytes);
            CutMaterialComponentSelectionV1 {
                component: component.key,
                faces,
                owns_original_boundary: boundary_components.contains(&component.key),
            }
        })
        .collect::<Vec<_>>();
    selections.sort_unstable_by_key(|selection| selection.component);
    if selections
        .iter()
        .all(|selection| selection.owns_original_boundary)
    {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let mut hash = Sha256::new();
    hash.update(CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1.as_bytes());
    hash.update(topology.fingerprint_v1());
    hash.update((selections.len() as u64).to_be_bytes());
    for selection in &selections {
        hash.update(selection.component.0);
        hash.update([u8::from(selection.owns_original_boundary)]);
        hash.update((selection.faces.len() as u64).to_be_bytes());
        for face in &selection.faces {
            hash.update(face.canonical_bytes());
        }
    }
    Ok(CutMaterialComponentSelectionDiagnosticV1 {
        selections,
        fingerprint: hash.finalize().into(),
        limits,
    })
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\"")).unwrap()
    }

    fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: id(suffix),
            position: Point2::new(x, y),
        }
    }

    fn edge(suffix: u64, a: &Vertex, b: &Vertex, kind: EdgeKind) -> Edge {
        Edge {
            id: id(suffix),
            start: a.id,
            end: b.id,
            kind,
        }
    }

    fn fixture(two_loops: bool) -> (ProjectId, Paper, CreasePattern) {
        let a = vertex(1, 0.0, 0.0);
        let b = vertex(2, 12.0, 0.0);
        let c = vertex(3, 12.0, 8.0);
        let d = vertex(4, 0.0, 8.0);
        let p = vertex(5, 2.0, 2.0);
        let q = vertex(6, 5.0, 2.0);
        let r = vertex(7, 3.5, 5.0);
        let s = vertex(8, 7.0, 2.0);
        let t = vertex(9, 10.0, 2.0);
        let u = vertex(10, 8.5, 5.0);
        let mut vertices = vec![
            a.clone(),
            b.clone(),
            c.clone(),
            d.clone(),
            p.clone(),
            q.clone(),
            r.clone(),
        ];
        let mut edges = vec![
            edge(20, &a, &b, EdgeKind::Boundary),
            edge(21, &b, &c, EdgeKind::Boundary),
            edge(22, &c, &d, EdgeKind::Boundary),
            edge(23, &d, &a, EdgeKind::Boundary),
            edge(30, &p, &q, EdgeKind::Cut),
            edge(31, &q, &r, EdgeKind::Cut),
            edge(32, &r, &p, EdgeKind::Cut),
        ];
        if two_loops {
            vertices.extend([s.clone(), t.clone(), u.clone()]);
            edges.extend([
                edge(33, &s, &t, EdgeKind::Cut),
                edge(34, &t, &u, EdgeKind::Cut),
                edge(35, &u, &s, EdgeKind::Cut),
            ]);
        }
        let paper = Paper {
            boundary_vertices: vec![a.id, b.id, c.id, d.id],
            cutting_allowed: true,
            ..Paper::default()
        };
        (id(100), paper, CreasePattern { vertices, edges })
    }

    fn input<'a>(
        namespace: ProjectId,
        revision: u64,
        paper: &'a Paper,
        pattern: &'a CreasePattern,
    ) -> FaceExtractionInput<'a> {
        FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: revision,
            paper,
            pattern,
        }
    }

    #[test]
    fn boundary_component_is_kept_and_inner_components_are_candidates_only() {
        for two_loops in [false, true] {
            let (namespace, paper, pattern) = fixture(two_loops);
            let source = input(namespace, 7, &paper, &pattern);
            let diagnostic =
                diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
            assert_eq!(
                diagnostic
                    .selections()
                    .iter()
                    .filter(|entry| entry.owns_original_boundary)
                    .count(),
                1
            );
            assert_eq!(
                diagnostic
                    .selections()
                    .iter()
                    .filter(|entry| !entry.owns_original_boundary)
                    .count(),
                if two_loops { 2 } else { 1 }
            );
            assert!(!diagnostic.authorizes_project_mutation());
            assert!(!diagnostic.authorizes_simulation_admission());
            assert!(diagnostic.is_for(source));
            assert!(!diagnostic.is_for(input(namespace, 8, &paper, &pattern)));
        }
    }

    #[test]
    fn pattern_paper_and_resource_tamper_fail_closed() {
        let (namespace, paper, pattern) = fixture(false);
        let source = input(namespace, 7, &paper, &pattern);
        let diagnostic =
            diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
        let mut changed = pattern.clone();
        changed.vertices[4].position.x += 0.25;
        assert!(!diagnostic.is_for(input(namespace, 7, &paper, &changed)));
        let mut changed_paper = paper.clone();
        changed_paper.thickness_mm = 0.2;
        assert!(!diagnostic.is_for(input(namespace, 7, &changed_paper, &pattern)));
        assert!(
            diagnose_cut_material_component_selection_v1(
                source,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: pattern.edges.len() - 1,
                    ..Default::default()
                },
            )
            .is_err()
        );
    }

    #[test]
    fn nested_loops_and_storage_reordering_are_canonical() {
        let (namespace, paper, mut pattern) = fixture(true);
        pattern.vertices[7].position = Point2::new(3.0, 2.5);
        pattern.vertices[8].position = Point2::new(4.0, 2.5);
        pattern.vertices[9].position = Point2::new(3.5, 3.5);
        let source = input(namespace, 7, &paper, &pattern);
        let first =
            diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
        assert_eq!(first.selections().len(), 3);
        assert_eq!(
            first
                .selections()
                .iter()
                .filter(|entry| entry.owns_original_boundary)
                .count(),
            1
        );
        assert_eq!(
            first
                .selections()
                .iter()
                .filter(|entry| !entry.owns_original_boundary)
                .count(),
            2
        );
        assert!(!first.authorizes_material_removal());

        let mut reordered = pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        let second = diagnose_cut_material_component_selection_v1(
            input(namespace, 7, &paper, &reordered),
            Default::default(),
        )
        .unwrap();
        assert_eq!(first.fingerprint_v1(), second.fingerprint_v1());
        assert_eq!(first.selections(), second.selections());
    }

    #[test]
    fn cut_loop_touching_original_boundary_fails_closed() {
        let (namespace, paper, mut pattern) = fixture(false);
        pattern.edges[4].start = paper.boundary_vertices[0];
        pattern.edges[6].end = paper.boundary_vertices[0];
        assert!(
            diagnose_cut_material_component_selection_v1(
                input(namespace, 7, &paper, &pattern),
                Default::default(),
            )
            .is_err()
        );
    }
}
