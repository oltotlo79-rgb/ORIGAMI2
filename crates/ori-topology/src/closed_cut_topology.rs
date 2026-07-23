use ori_domain::{LengthDisplayUnit, Paper, PaperAppearance};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

use crate::{
    ClosedCutLoopDiagnosticErrorV1, ClosedCutLoopDiagnosticLimitsV1, CooperativeAnalysisCheckpoint,
    CooperativeOperationError, FaceExtractionInput, TopologySnapshot,
    closed_cut::diagnose_closed_cut_loops_v1,
    fold_graph::extract_fold_graph_snapshot_with_checkpoint,
};

pub const CLOSED_CUT_TOPOLOGY_SNAPSHOT_MODEL_ID_V1: &str =
    "closed_cut_topology_snapshot_diagnostic_v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClosedCutTopologySnapshotErrorV1 {
    #[error("closed-cut prerequisite failed: {0}")]
    ClosedCut(#[from] ClosedCutLoopDiagnosticErrorV1),
    #[error("closed-cut topology extraction failed closed")]
    InvalidTopology,
    #[error("closed-cut topology binding could not be represented")]
    ResourceLimit,
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

    fn fixture() -> (ProjectId, Paper, CreasePattern) {
        let a = vertex(1, 0.0, 0.0);
        let b = vertex(2, 8.0, 0.0);
        let c = vertex(3, 8.0, 8.0);
        let d = vertex(4, 0.0, 8.0);
        let p = vertex(5, 2.0, 2.0);
        let q = vertex(6, 6.0, 2.0);
        let r = vertex(7, 4.0, 6.0);
        let vertices = vec![
            a.clone(),
            b.clone(),
            c.clone(),
            d.clone(),
            p.clone(),
            q.clone(),
            r.clone(),
        ];
        let edges = vec![
            edge(10, &a, &b, EdgeKind::Boundary),
            edge(11, &b, &c, EdgeKind::Boundary),
            edge(12, &c, &d, EdgeKind::Boundary),
            edge(13, &d, &a, EdgeKind::Boundary),
            edge(20, &p, &q, EdgeKind::Cut),
            edge(21, &q, &r, EdgeKind::Cut),
            edge(22, &r, &p, EdgeKind::Cut),
        ];
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
    fn closed_loop_yields_read_only_hole_snapshot_and_preserves_public_rejection() {
        let (namespace, paper, pattern) = fixture();
        let source = input(namespace, 9, &paper, &pattern);
        let diagnostic =
            diagnose_closed_cut_topology_snapshot_v1(source, Default::default()).unwrap();
        assert_eq!(diagnostic.snapshot().faces.len(), 2);
        assert_eq!(diagnostic.snapshot().material_components.len(), 2);
        assert_eq!(
            diagnostic
                .snapshot()
                .faces
                .iter()
                .filter(|face| face.holes.len() == 1)
                .count(),
            1
        );
        assert!(!diagnostic.authorizes_simulation_admission());
        assert!(!diagnostic.authorizes_project_mutation());
        let ordinary = crate::analyze_faces(source);
        assert!(ordinary.snapshot.is_none());
        assert!(matches!(
            ordinary.issues[0].kind,
            crate::TopologyIssueKind::UnsupportedActiveEdge {
                edge_kind: EdgeKind::Cut,
                ..
            }
        ));
    }

    #[test]
    fn binding_and_resource_caps_fail_closed() {
        let (namespace, paper, pattern) = fixture();
        let source = input(namespace, 9, &paper, &pattern);
        let diagnostic =
            diagnose_closed_cut_topology_snapshot_v1(source, Default::default()).unwrap();
        assert!(diagnostic.is_for(source));
        assert!(!diagnostic.is_for(input(namespace, 10, &paper, &pattern)));
        assert!(!diagnostic.is_for(input(id(101), 9, &paper, &pattern)));
        let mut changed_paper = paper.clone();
        changed_paper.thickness_mm = 0.2;
        assert!(!diagnostic.is_for(input(namespace, 9, &changed_paper, &pattern)));
        let mut changed_pattern = pattern.clone();
        changed_pattern.vertices[4].position.x += 0.25;
        assert!(!diagnostic.is_for(input(namespace, 9, &paper, &changed_pattern)));
        let limits = ClosedCutLoopDiagnosticLimitsV1 {
            max_vertices: pattern.vertices.len() - 1,
            ..Default::default()
        };
        assert!(matches!(
            diagnose_closed_cut_topology_snapshot_v1(source, limits),
            Err(ClosedCutTopologySnapshotErrorV1::ClosedCut(
                ClosedCutLoopDiagnosticErrorV1::ResourceLimit
            ))
        ));
        let mut oversized_paper = paper.clone();
        oversized_paper.boundary_vertices = vec![pattern.vertices[0].id; limits.max_vertices + 1];
        assert_eq!(
            diagnose_closed_cut_topology_snapshot_v1(
                input(namespace, 9, &oversized_paper, &pattern),
                limits
            ),
            Err(ClosedCutTopologySnapshotErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn open_chain_crossing_and_no_cut_are_rejected() {
        let (namespace, paper, pattern) = fixture();
        let mut chain = pattern.clone();
        chain.edges.pop();
        assert!(
            diagnose_closed_cut_topology_snapshot_v1(
                input(namespace, 9, &paper, &chain),
                Default::default()
            )
            .is_err()
        );

        let mut crossing = pattern.clone();
        let s = vertex(30, 2.0, 6.0);
        let t = vertex(31, 6.0, 6.0);
        crossing.vertices.extend([s.clone(), t.clone()]);
        crossing.edges.extend([
            edge(32, &s, &t, EdgeKind::Cut),
            edge(33, &t, &crossing.vertices[4], EdgeKind::Cut),
            edge(34, &crossing.vertices[4], &s, EdgeKind::Cut),
        ]);
        assert!(
            diagnose_closed_cut_topology_snapshot_v1(
                input(namespace, 9, &paper, &crossing),
                Default::default()
            )
            .is_err()
        );

        let mut no_cut = pattern.clone();
        no_cut.edges.truncate(4);
        assert!(matches!(
            diagnose_closed_cut_topology_snapshot_v1(
                input(namespace, 9, &paper, &no_cut),
                Default::default()
            ),
            Err(ClosedCutTopologySnapshotErrorV1::ClosedCut(
                ClosedCutLoopDiagnosticErrorV1::NotClosedLoop
            ))
        ));
    }

    #[test]
    fn radial_fold_at_a_cut_vertex_remains_fail_closed() {
        let (namespace, paper, mut pattern) = fixture();
        let fold = edge(
            40,
            &pattern.vertices[5],
            &pattern.vertices[1],
            EdgeKind::Mountain,
        );
        pattern.edges.push(fold);
        assert_eq!(
            diagnose_closed_cut_topology_snapshot_v1(
                input(namespace, 9, &paper, &pattern),
                Default::default(),
            ),
            Err(ClosedCutTopologySnapshotErrorV1::InvalidTopology)
        );
    }
}

/// Read-only view of face subdivision behind the closed-cut-loop prerequisite.
///
/// This object deliberately carries no admission authority. In particular,
/// the inner face is still a detached material piece: a hole in the outer face
/// does not prove that material was removed or that a relief exists. Downstream
/// kinematics must continue to reject faces containing holes. A radial hinge
/// ending at a cut-loop vertex remains unsupported and fails closed.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedCutTopologySnapshotDiagnosticV1 {
    snapshot: TopologySnapshot,
    fingerprint: [u8; 32],
    limits: ClosedCutLoopDiagnosticLimitsV1,
    paper: Paper,
}

impl ClosedCutTopologySnapshotDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CLOSED_CUT_TOPOLOGY_SNAPSHOT_MODEL_ID_V1
    }

    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    /// Returns a non-authoritative observation. Possessing or cloning this
    /// snapshot never bypasses ordinary topology or kinematics admission.
    pub const fn snapshot(&self) -> &TopologySnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub fn is_for(&self, input: FaceExtractionInput<'_>) -> bool {
        self.paper == *input.paper
            && diagnose_closed_cut_topology_snapshot_v1(input, self.limits)
                .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

pub fn diagnose_closed_cut_topology_snapshot_v1(
    input: FaceExtractionInput<'_>,
    limits: ClosedCutLoopDiagnosticLimitsV1,
) -> Result<ClosedCutTopologySnapshotDiagnosticV1, ClosedCutTopologySnapshotErrorV1> {
    if input.paper.boundary_vertices.len() > limits.max_vertices {
        return Err(ClosedCutTopologySnapshotErrorV1::ResourceLimit);
    }
    let closed = diagnose_closed_cut_loops_v1(input.pattern, limits)?;
    let mut checkpoint = || CooperativeAnalysisCheckpoint::Continue;
    let snapshot = match extract_fold_graph_snapshot_with_checkpoint(input, &mut checkpoint) {
        Ok(snapshot) => snapshot,
        Err(CooperativeOperationError::Operation(_))
        | Err(CooperativeOperationError::Aborted(_)) => {
            return Err(ClosedCutTopologySnapshotErrorV1::InvalidTopology);
        }
    };
    // A successful closed-loop prerequisite must be reflected completely as a face
    // subdivision: an outer component with a hole plus the detached inner
    // material component. This is not a cutout/removal certificate.
    let loop_count = closed.loops().len();
    let hole_count = snapshot
        .faces
        .iter()
        .map(|face| face.holes.len())
        .sum::<usize>();
    let mut cut_boundary_occurrences = HashMap::new();
    for face in &snapshot.faces {
        for boundary in std::iter::once(&face.outer).chain(&face.holes) {
            for half_edge in &boundary.half_edges {
                *cut_boundary_occurrences
                    .entry(half_edge.edge)
                    .or_insert(0usize) += 1;
            }
        }
    }
    let every_cut_is_a_two_sided_subdivision = closed.loops().iter().flatten().all(|edge| {
        cut_boundary_occurrences.get(edge) == Some(&2)
            && snapshot
                .edge_incidence
                .iter()
                .any(|(candidate, incidence)| {
                    candidate == edge
                        && matches!(
                            incidence,
                            crate::EdgeIncidence::Cut { left, right } if left != right
                        )
                })
    });
    if loop_count == 0
        || hole_count != loop_count
        || snapshot.material_components.len() != loop_count.saturating_add(1)
        || snapshot.faces.len() < loop_count.saturating_add(1)
        || snapshot.faces.iter().any(|face| !face.seams.is_empty())
        || !every_cut_is_a_two_sided_subdivision
    {
        return Err(ClosedCutTopologySnapshotErrorV1::InvalidTopology);
    }

    let mut hash = Sha256::new();
    hash.update(CLOSED_CUT_TOPOLOGY_SNAPSHOT_MODEL_ID_V1.as_bytes());
    hash.update(closed.fingerprint_v1());
    hash.update(input.identity_namespace.canonical_bytes());
    hash.update(input.source_revision.to_be_bytes());
    hash_paper(&mut hash, input.paper);
    hash.update((limits.max_vertices as u64).to_be_bytes());
    hash.update((limits.max_edges as u64).to_be_bytes());
    hash.update((limits.max_intersection_tests as u64).to_be_bytes());
    Ok(ClosedCutTopologySnapshotDiagnosticV1 {
        snapshot,
        fingerprint: hash.finalize().into(),
        limits,
        paper: input.paper.clone(),
    })
}

fn hash_paper(hash: &mut Sha256, paper: &Paper) {
    hash.update((paper.boundary_vertices.len() as u64).to_be_bytes());
    for vertex in &paper.boundary_vertices {
        hash.update(vertex.canonical_bytes());
    }
    hash.update(paper.thickness_mm.to_bits().to_be_bytes());
    match paper.length_display_unit {
        LengthDisplayUnit::Millimeter => hash.update([0]),
        LengthDisplayUnit::Centimeter => hash.update([1]),
        LengthDisplayUnit::Inch => hash.update([2]),
        LengthDisplayUnit::PaperEdgeRatio { reference_edge } => {
            hash.update([3]);
            hash.update(reference_edge.canonical_bytes());
        }
    }
    hash.update([u8::from(paper.cutting_allowed)]);
    for appearance in [&paper.front, &paper.back] {
        hash_appearance(hash, appearance);
    }
}

fn hash_appearance(hash: &mut Sha256, appearance: &PaperAppearance) {
    hash.update([
        appearance.color.red,
        appearance.color.green,
        appearance.color.blue,
        appearance.color.alpha,
    ]);
    match appearance.texture_asset {
        Some(asset) => {
            hash.update([1]);
            hash.update(asset.canonical_bytes());
        }
        None => hash.update([0]),
    }
}
