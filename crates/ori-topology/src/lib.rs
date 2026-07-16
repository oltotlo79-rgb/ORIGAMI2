//! Deterministic planar topology derived from an ORIGAMI2 crease pattern.
//!
//! The first implementation slice intentionally accepts boundary-only sheets.
//! It establishes stable face identity and a strict diagnostic boundary before
//! fold and cut edges are admitted into the half-edge reconstruction.

use std::collections::{HashMap, HashSet};

use ori_domain::{CreasePattern, EdgeId, EdgeKind, FaceId, Paper, ProjectId, VertexId};
use ori_geometry::{polygon_signed_double_area, validate_paper};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const FACE_KEY_DOMAIN: &[u8] = b"ORIGAMI2_FACE_KEY_V1";

/// Immutable input used to derive a topology snapshot.
#[derive(Debug, Clone, Copy)]
pub struct FaceExtractionInput<'a> {
    pub identity_namespace: ProjectId,
    pub source_revision: u64,
    pub paper: &'a Paper,
    pub pattern: &'a CreasePattern,
}

/// Canonical SHA-256 digest of one material face boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FaceKey(pub [u8; 32]);

/// One directed occurrence of a source edge in a face boundary walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HalfEdgeRef {
    pub edge: EdgeId,
    pub origin: VertexId,
    pub destination: VertexId,
}

/// A counter-clockwise walk whose material lies on its left side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryWalk {
    pub half_edges: Vec<HalfEdgeRef>,
    pub signed_double_area: f64,
}

/// One connected two-dimensional material region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub id: FaceId,
    pub key: FaceKey,
    pub outer: BoundaryWalk,
    pub area: f64,
}

/// The relation of a source edge to the extracted material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EdgeIncidence {
    Boundary { material: FaceId },
    AuxiliaryIgnored,
}

/// Canonical, revision-labelled output of face extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologySnapshot {
    pub source_revision: u64,
    pub faces: Vec<Face>,
    pub edge_incidence: Vec<(EdgeId, EdgeIncidence)>,
}

/// Whether a topology issue permits downstream simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyIssueSeverity {
    Warning,
    BlocksSimulation,
    Fatal,
}

/// Machine-readable reason that a topology snapshot was not produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TopologyIssueKind {
    DuplicateVertexId { vertex: VertexId },
    DuplicateEdgeId { edge: EdgeId },
    InvalidPaper { issue_count: usize },
    UnsupportedActiveEdge { edge: EdgeId, edge_kind: EdgeKind },
    UnrepresentableFaceArea,
    InternalBoundaryResolution,
}

/// One stable diagnostic returned by face analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyIssue {
    pub severity: TopologyIssueSeverity,
    pub kind: TopologyIssueKind,
}

/// Diagnostic result used by the editor while the document may be invalid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceExtractionReport {
    pub snapshot: Option<TopologySnapshot>,
    pub issues: Vec<TopologyIssue>,
}

/// Strict extraction rejected an input that is not safe for simulation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("face extraction was rejected by {issue_count} topology issue(s)")]
pub struct FaceExtractionRejected {
    issue_count: usize,
    pub issues: Vec<TopologyIssue>,
}

impl FaceExtractionRejected {
    #[must_use]
    pub fn issue_count(&self) -> usize {
        self.issue_count
    }
}

/// Analyzes the document without mutating it.
///
/// Mountain, valley, and cut edges are rejected explicitly in this first
/// boundary-only slice. They will be enabled when the DCEL walk grouping is
/// implemented; silently treating them as auxiliary would create unsafe 3D
/// input.
#[must_use]
pub fn analyze_faces(input: FaceExtractionInput<'_>) -> FaceExtractionReport {
    let mut vertex_ids = HashSet::with_capacity(input.pattern.vertices.len());
    let mut duplicate_vertex = None;
    for vertex in &input.pattern.vertices {
        if !vertex_ids.insert(vertex.id)
            && duplicate_vertex.is_none_or(|current: VertexId| {
                vertex.id.canonical_bytes() < current.canonical_bytes()
            })
        {
            duplicate_vertex = Some(vertex.id);
        }
    }
    if let Some(vertex) = duplicate_vertex {
        return rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::DuplicateVertexId { vertex },
        );
    }

    let mut edge_ids = HashSet::with_capacity(input.pattern.edges.len());
    let mut duplicate_edge = None;
    for edge in &input.pattern.edges {
        if !edge_ids.insert(edge.id)
            && duplicate_edge
                .is_none_or(|current: EdgeId| edge.id.canonical_bytes() < current.canonical_bytes())
        {
            duplicate_edge = Some(edge.id);
        }
    }
    if let Some(edge) = duplicate_edge {
        return rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::DuplicateEdgeId { edge },
        );
    }

    let paper_validation = validate_paper(input.paper, input.pattern);
    if !paper_validation.is_valid() {
        return rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InvalidPaper {
                issue_count: paper_validation.issues.len(),
            },
        );
    }

    if let Some(edge) = input
        .pattern
        .edges
        .iter()
        .filter(|edge| !matches!(edge.kind, EdgeKind::Boundary | EdgeKind::Auxiliary))
        .min_by_key(|edge| edge.id.canonical_bytes())
    {
        return rejected(
            TopologyIssueSeverity::BlocksSimulation,
            TopologyIssueKind::UnsupportedActiveEdge {
                edge: edge.id,
                edge_kind: edge.kind,
            },
        );
    }

    match extract_boundary_face(input) {
        Ok(snapshot) => FaceExtractionReport {
            snapshot: Some(snapshot),
            issues: Vec::new(),
        },
        Err(kind) => rejected(TopologyIssueSeverity::Fatal, kind),
    }
}

/// Returns a topology snapshot only when no simulation-blocking issue exists.
pub fn extract_faces_strict(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, FaceExtractionRejected> {
    let report = analyze_faces(input);
    if report.issues.iter().any(|issue| {
        matches!(
            issue.severity,
            TopologyIssueSeverity::BlocksSimulation | TopologyIssueSeverity::Fatal
        )
    }) {
        return Err(FaceExtractionRejected {
            issue_count: report.issues.len(),
            issues: report.issues,
        });
    }
    report.snapshot.ok_or_else(|| FaceExtractionRejected {
        issue_count: 1,
        issues: vec![TopologyIssue {
            severity: TopologyIssueSeverity::Fatal,
            kind: TopologyIssueKind::InternalBoundaryResolution,
        }],
    })
}

fn rejected(severity: TopologyIssueSeverity, kind: TopologyIssueKind) -> FaceExtractionReport {
    FaceExtractionReport {
        snapshot: None,
        issues: vec![TopologyIssue { severity, kind }],
    }
}

fn extract_boundary_face(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, TopologyIssueKind> {
    let positions = input
        .pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let mut boundary_vertices = input.paper.boundary_vertices.clone();
    let boundary_positions = boundary_vertices
        .iter()
        .map(|vertex| positions.get(vertex).copied())
        .collect::<Option<Vec<_>>>()
        .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
    let signed_double_area = polygon_signed_double_area(&boundary_positions)
        .map_err(|_| TopologyIssueKind::InternalBoundaryResolution)?;
    if signed_double_area == 0.0 {
        return Err(TopologyIssueKind::InternalBoundaryResolution);
    }
    if signed_double_area < 0.0 {
        boundary_vertices.reverse();
    }

    let mut half_edges = Vec::with_capacity(boundary_vertices.len());
    for index in 0..boundary_vertices.len() {
        let origin = boundary_vertices[index];
        let destination = boundary_vertices[(index + 1) % boundary_vertices.len()];
        let edge = input
            .pattern
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Boundary
                    && ((edge.start == origin && edge.end == destination)
                        || (edge.start == destination && edge.end == origin))
            })
            .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
        half_edges.push(HalfEdgeRef {
            edge: edge.id,
            origin,
            destination,
        });
    }
    canonicalize_cycle(&mut half_edges);

    let key = face_key(&half_edges);
    let face_id = FaceId::derive_v5(input.identity_namespace, &key.0);
    let area = signed_double_area.abs() * 0.5;
    if area == 0.0 || !area.is_finite() {
        return Err(TopologyIssueKind::UnrepresentableFaceArea);
    }
    let face = Face {
        id: face_id,
        key,
        outer: BoundaryWalk {
            half_edges,
            signed_double_area: signed_double_area.abs(),
        },
        area,
    };

    let mut edge_incidence = input
        .pattern
        .edges
        .iter()
        .map(|edge| {
            let incidence = match edge.kind {
                EdgeKind::Boundary => EdgeIncidence::Boundary { material: face_id },
                EdgeKind::Auxiliary => EdgeIncidence::AuxiliaryIgnored,
                EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut => return None,
            };
            Some((edge.id, incidence))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
    edge_incidence.sort_by_key(|(edge, _)| edge.canonical_bytes());

    Ok(TopologySnapshot {
        source_revision: input.source_revision,
        faces: vec![face],
        edge_incidence,
    })
}

fn canonicalize_cycle(half_edges: &mut [HalfEdgeRef]) {
    let tokens = half_edges.iter().map(half_edge_token).collect::<Vec<_>>();
    let best = (1..tokens.len()).fold(0, |best, candidate| {
        if rotation_is_less(&tokens, candidate, best) {
            candidate
        } else {
            best
        }
    });
    half_edges.rotate_left(best);
}

fn half_edge_token(half_edge: &HalfEdgeRef) -> [u8; 48] {
    let mut token = [0_u8; 48];
    token[..16].copy_from_slice(&half_edge.edge.canonical_bytes());
    token[16..32].copy_from_slice(&half_edge.origin.canonical_bytes());
    token[32..].copy_from_slice(&half_edge.destination.canonical_bytes());
    token
}

fn rotation_is_less(tokens: &[[u8; 48]], candidate: usize, current: usize) -> bool {
    (0..tokens.len())
        .map(|offset| {
            (
                &tokens[(candidate + offset) % tokens.len()],
                &tokens[(current + offset) % tokens.len()],
            )
        })
        .find_map(|(left, right)| (left != right).then_some(left < right))
        .unwrap_or(false)
}

fn face_key(half_edges: &[HalfEdgeRef]) -> FaceKey {
    let mut hasher = Sha256::new();
    hasher.update(FACE_KEY_DOMAIN);
    hasher.update((half_edges.len() as u64).to_be_bytes());
    for half_edge in half_edges {
        hasher.update(half_edge_token(half_edge));
    }
    FaceKey(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, Paper, Point2, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn polygon_fixture(points: &[Point2]) -> (ProjectId, Paper, CreasePattern) {
        let namespace = ProjectId::new();
        let vertices = points
            .iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position: *position,
            })
            .collect::<Vec<_>>();
        let boundary_vertices = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices,
            ..Paper::default()
        };
        (namespace, paper, CreasePattern { vertices, edges })
    }

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn strict(namespace: ProjectId, paper: &Paper, pattern: &CreasePattern) -> TopologySnapshot {
        extract_faces_strict(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 7,
            paper,
            pattern,
        })
        .expect("extract boundary-only topology")
    }

    #[test]
    fn square_extracts_one_face_and_sorted_boundary_incidence() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ]);

        let snapshot = strict(namespace, &paper, &pattern);

        assert_eq!(snapshot.source_revision, 7);
        assert_eq!(snapshot.faces.len(), 1);
        assert_eq!(snapshot.faces[0].area, 100.0);
        assert_eq!(snapshot.faces[0].outer.signed_double_area, 200.0);
        assert_eq!(snapshot.faces[0].outer.half_edges.len(), 4);
        assert!(snapshot.edge_incidence.iter().all(|(_, incidence)| {
            matches!(incidence, EdgeIncidence::Boundary { material } if *material == snapshot.faces[0].id)
        }));
        assert!(
            snapshot
                .edge_incidence
                .windows(2)
                .all(|pair| pair[0].0.canonical_bytes() < pair[1].0.canonical_bytes())
        );
    }

    #[test]
    fn stable_face_identity_matches_the_v1_golden_vector() {
        let namespace = fixed_id(1);
        let vertex_ids = [
            fixed_id(0x101),
            fixed_id(0x102),
            fixed_id(0x103),
            fixed_id(0x104),
        ];
        let edge_ids = [
            fixed_id(0x201),
            fixed_id(0x202),
            fixed_id(0x203),
            fixed_id(0x204),
        ];
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ];
        let vertices = vertex_ids
            .iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id: *id, position })
            .collect::<Vec<_>>();
        let edges = (0..4)
            .map(|index| Edge {
                id: edge_ids[index],
                start: vertex_ids[index],
                end: vertex_ids[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices: vertex_ids.to_vec(),
            ..Paper::default()
        };
        let pattern = CreasePattern { vertices, edges };

        let face = &strict(namespace, &paper, &pattern).faces[0];

        assert_eq!(
            (face.key.0, face.id.canonical_bytes()),
            (
                [
                    34, 113, 28, 115, 75, 184, 146, 47, 25, 14, 93, 61, 99, 234, 88, 143, 156, 135,
                    97, 14, 14, 143, 237, 92, 65, 97, 20, 62, 98, 234, 24, 11,
                ],
                [
                    167, 199, 122, 53, 123, 194, 89, 121, 169, 212, 34, 191, 48, 106, 196, 29,
                ],
            )
        );
    }

    #[test]
    fn concave_boundary_extracts_its_exact_area() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 4.0),
        ]);

        let snapshot = strict(namespace, &paper, &pattern);

        assert_eq!(snapshot.faces[0].area, 12.0);
    }

    #[test]
    fn canonical_snapshot_ignores_each_equivalent_storage_transform() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(2.5, -0.2),
            Point2::new(0.0, 3.8),
            Point2::new(-2.5, 0.3),
            Point2::new(0.3, -3.2),
        ]);
        let expected = strict(namespace, &paper, &pattern);

        let mut cyclic_paper = paper.clone();
        cyclic_paper.boundary_vertices.rotate_left(1);
        assert_eq!(strict(namespace, &cyclic_paper, &pattern), expected);

        let mut clockwise_paper = paper.clone();
        clockwise_paper.boundary_vertices.reverse();
        assert_eq!(strict(namespace, &clockwise_paper, &pattern), expected);

        let mut reversed_edges = pattern.clone();
        for edge in &mut reversed_edges.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        assert_eq!(strict(namespace, &paper, &reversed_edges), expected);

        let mut reordered_records = pattern.clone();
        reordered_records.vertices.reverse();
        reordered_records.edges.reverse();
        assert_eq!(strict(namespace, &paper, &reordered_records), expected);
    }

    #[test]
    fn face_identity_and_area_are_invariant_under_large_exact_translation() {
        let (namespace, paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.0, 1.0),
        ]);
        let expected = strict(namespace, &paper, &pattern);
        for vertex in &mut pattern.vertices {
            vertex.position.x += 1_000_000_000_000.0;
            vertex.position.y -= 1_000_000_000_000.0;
        }

        let translated = strict(namespace, &paper, &pattern);

        assert_eq!(translated, expected);
        assert_eq!(translated.faces[0].area, 1.0);
    }

    #[test]
    fn face_extraction_rejects_area_that_underflows_when_halved() {
        let side = f64::from_bits(486_u64 << 52);
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(side, 0.0),
            Point2::new(0.0, side),
        ]);

        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &pattern,
        });

        assert!(report.snapshot.is_none());
        assert_eq!(
            report.issues,
            vec![TopologyIssue {
                severity: TopologyIssueSeverity::Fatal,
                kind: TopologyIssueKind::UnrepresentableFaceArea,
            }]
        );
    }

    #[test]
    fn auxiliary_edges_do_not_change_face_identity() {
        let (namespace, paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);
        let baseline = strict(namespace, &paper, &pattern);
        let auxiliary_id = EdgeId::new();
        pattern.edges.push(Edge {
            id: auxiliary_id,
            start: pattern.vertices[0].id,
            end: pattern.vertices[2].id,
            kind: EdgeKind::Auxiliary,
        });

        let with_auxiliary = strict(namespace, &paper, &pattern);

        assert_eq!(with_auxiliary.faces, baseline.faces);
        assert_eq!(
            with_auxiliary
                .edge_incidence
                .iter()
                .find(|(edge, _)| *edge == auxiliary_id)
                .map(|(_, value)| value),
            Some(&EdgeIncidence::AuxiliaryIgnored)
        );
    }

    #[test]
    fn active_fold_and_cut_edges_are_explicitly_blocked() {
        for kind in [EdgeKind::Mountain, EdgeKind::Valley, EdgeKind::Cut] {
            let (namespace, mut paper, mut pattern) = polygon_fixture(&[
                Point2::new(0.0, 0.0),
                Point2::new(5.0, 0.0),
                Point2::new(5.0, 5.0),
                Point2::new(0.0, 5.0),
            ]);
            paper.cutting_allowed = true;
            let edge_id = EdgeId::new();
            pattern.edges.push(Edge {
                id: edge_id,
                start: pattern.vertices[0].id,
                end: pattern.vertices[2].id,
                kind,
            });

            let error = extract_faces_strict(FaceExtractionInput {
                identity_namespace: namespace,
                source_revision: 0,
                paper: &paper,
                pattern: &pattern,
            })
            .expect_err("active edges are not implemented in the first slice");

            assert_eq!(error.issue_count(), 1);
            assert_eq!(
                error.issues[0],
                TopologyIssue {
                    severity: TopologyIssueSeverity::BlocksSimulation,
                    kind: TopologyIssueKind::UnsupportedActiveEdge {
                        edge: edge_id,
                        edge_kind: kind,
                    },
                }
            );
        }
    }

    #[test]
    fn invalid_paper_and_duplicate_ids_fail_closed() {
        let (namespace, mut paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);
        paper.boundary_vertices.pop();
        let invalid = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(invalid.snapshot.is_none());
        assert!(matches!(
            invalid.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));

        let duplicate = pattern.vertices[0].clone();
        pattern.vertices.push(duplicate.clone());
        let duplicate_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &Paper {
                boundary_vertices: pattern.vertices[..4]
                    .iter()
                    .map(|vertex| vertex.id)
                    .collect(),
                ..Paper::default()
            },
            pattern: &pattern,
        });
        assert_eq!(
            duplicate_report.issues[0].kind,
            TopologyIssueKind::DuplicateVertexId {
                vertex: duplicate.id
            }
        );
    }

    #[test]
    fn duplicate_edges_missing_endpoints_and_non_finite_vertices_fail_closed() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);

        let mut duplicate_edges = pattern.clone();
        duplicate_edges.edges.push(pattern.edges[0].clone());
        let duplicate_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &duplicate_edges,
        });
        assert_eq!(
            duplicate_report.issues[0].kind,
            TopologyIssueKind::DuplicateEdgeId {
                edge: pattern.edges[0].id
            }
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint.edges[0].end = VertexId::new();
        let missing_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &missing_endpoint,
        });
        assert!(matches!(
            missing_report.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));

        let mut non_finite = pattern.clone();
        non_finite.vertices[0].position.x = f64::NAN;
        let non_finite_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &non_finite,
        });
        assert!(matches!(
            non_finite_report.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));
    }
}
