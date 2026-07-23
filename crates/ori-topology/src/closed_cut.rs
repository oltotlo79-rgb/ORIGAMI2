use std::collections::{HashMap, HashSet};

use ori_domain::{CreasePattern, EdgeId, EdgeKind, Point2, VertexId};
use ori_geometry::{SegmentIntersection, segment_intersection};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CLOSED_CUT_LOOP_DIAGNOSTIC_MODEL_ID_V1: &str = "canonical_closed_cut_loop_diagnostic_v1";
pub const MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1: usize = 1_024;
pub const MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1: usize = 1_024;
pub const MAX_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1: usize =
    MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1 * MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1;
pub const DEFAULT_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1: usize = 262_144;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedCutLoopDiagnosticLimitsV1 {
    pub max_vertices: usize,
    pub max_edges: usize,
    pub max_intersection_tests: usize,
}

impl Default for ClosedCutLoopDiagnosticLimitsV1 {
    fn default() -> Self {
        Self {
            max_vertices: MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1,
            max_edges: MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1,
            max_intersection_tests: DEFAULT_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClosedCutLoopDiagnosticErrorV1 {
    #[error("closed-cut diagnostic limits are invalid")]
    InvalidLimit,
    #[error("closed-cut diagnostic resource limit exceeded")]
    ResourceLimit,
    #[error("cut graph is not a disjoint union of simple closed loops")]
    NotClosedLoop,
    #[error("cut loop geometry is invalid")]
    InvalidGeometry,
    #[error("cut loop intersects another material edge")]
    MaterialEdgeIntersection,
}

/// Opaque one-shot observation of isolated existing `EdgeKind::Cut` loops.
/// V1 permits only identity-sharing endpoint contact with a non-cut edge,
/// which is the representation required after splitting a radial crease at a
/// relief boundary. It remains a foundation, not a vertex-relief admission.
/// It does not admit cut topology or authorize project mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedCutLoopDiagnosticV1 {
    loops: Vec<Vec<EdgeId>>,
    fingerprint: [u8; 32],
}

impl ClosedCutLoopDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CLOSED_CUT_LOOP_DIAGNOSTIC_MODEL_ID_V1
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
    pub fn loops(&self) -> &[Vec<EdgeId>] {
        &self.loops
    }

    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub fn is_for(&self, pattern: &CreasePattern, limits: ClosedCutLoopDiagnosticLimitsV1) -> bool {
        diagnose_closed_cut_loops_v1(pattern, limits)
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

pub fn diagnose_closed_cut_loops_v1(
    pattern: &CreasePattern,
    limits: ClosedCutLoopDiagnosticLimitsV1,
) -> Result<ClosedCutLoopDiagnosticV1, ClosedCutLoopDiagnosticErrorV1> {
    if limits.max_vertices > MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1
        || limits.max_edges > MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1
        || limits.max_intersection_tests > MAX_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1
    {
        return Err(ClosedCutLoopDiagnosticErrorV1::InvalidLimit);
    }
    if pattern.vertices.len() > limits.max_vertices || pattern.edges.len() > limits.max_edges {
        return Err(ClosedCutLoopDiagnosticErrorV1::ResourceLimit);
    }
    let mut positions = HashMap::new();
    positions
        .try_reserve(pattern.vertices.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    positions.extend(
        pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position)),
    );
    if positions.len() != pattern.vertices.len()
        || positions
            .values()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
    }
    let mut edge_ids = HashSet::new();
    edge_ids
        .try_reserve(pattern.edges.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    edge_ids.extend(pattern.edges.iter().map(|edge| edge.id));
    if edge_ids.len() != pattern.edges.len() {
        return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
    }
    let mut cuts = Vec::new();
    cuts.try_reserve_exact(pattern.edges.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    cuts.extend(
        pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut),
    );
    if cuts.is_empty() {
        return Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop);
    }
    let mut adjacency = HashMap::<VertexId, Vec<(VertexId, EdgeId)>>::new();
    adjacency
        .try_reserve(cuts.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    for edge in &cuts {
        let Some(start) = positions.get(&edge.start).copied() else {
            return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
        };
        let Some(end) = positions.get(&edge.end).copied() else {
            return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
        };
        if edge.start == edge.end || start == end {
            return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
        }
        for (from, to) in [(edge.start, edge.end), (edge.end, edge.start)] {
            let neighbors = adjacency.entry(from).or_default();
            neighbors
                .try_reserve(1)
                .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
            neighbors.push((to, edge.id));
        }
    }
    if adjacency.values().any(|neighbors| neighbors.len() != 2) {
        return Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop);
    }
    let mut remaining = HashSet::new();
    remaining
        .try_reserve(cuts.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    remaining.extend(cuts.iter().map(|edge| edge.id));
    let mut loops = Vec::new();
    loops
        .try_reserve(cuts.len() / 3)
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    while let Some(&seed) = remaining.iter().min_by_key(|edge| edge.canonical_bytes()) {
        let seed_edge = cuts
            .iter()
            .find(|edge| edge.id == seed)
            .ok_or(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)?;
        let start = if seed_edge.start.canonical_bytes() <= seed_edge.end.canonical_bytes() {
            seed_edge.start
        } else {
            seed_edge.end
        };
        let mut current = start;
        let mut previous = None;
        let mut loop_edges = Vec::new();
        loop_edges
            .try_reserve(cuts.len())
            .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
        loop {
            let next = adjacency
                .get(&current)
                .and_then(|neighbors| {
                    neighbors
                        .iter()
                        .filter(|(_, edge)| Some(*edge) != previous)
                        .min_by_key(|(vertex, edge)| {
                            (edge.canonical_bytes(), vertex.canonical_bytes())
                        })
                })
                .copied()
                .ok_or(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop)?;
            if !remaining.remove(&next.1) {
                return Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop);
            }
            loop_edges.push(next.1);
            previous = Some(next.1);
            current = next.0;
            if current == start {
                break;
            }
        }
        if loop_edges.len() < 3 {
            return Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop);
        }
        let canonical_start = loop_edges
            .iter()
            .enumerate()
            .min_by_key(|(_, edge)| edge.canonical_bytes())
            .map(|(index, _)| index)
            .unwrap_or(0);
        loop_edges.rotate_left(canonical_start);
        loops.push(loop_edges);
    }
    loops.sort_unstable_by_key(|edges| edges[0].canonical_bytes());
    validate_intersections(pattern, &positions, &cuts, limits.max_intersection_tests)?;
    let mut hash = Sha256::new();
    hash.update(CLOSED_CUT_LOOP_DIAGNOSTIC_MODEL_ID_V1.as_bytes());
    let mut all_vertices = Vec::new();
    all_vertices
        .try_reserve_exact(pattern.vertices.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    all_vertices.extend(&pattern.vertices);
    all_vertices.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    hash.update((all_vertices.len() as u64).to_be_bytes());
    for vertex in all_vertices {
        hash.update(vertex.id.canonical_bytes());
        hash.update(vertex.position.x.to_bits().to_be_bytes());
        hash.update(vertex.position.y.to_bits().to_be_bytes());
    }
    let mut all_edges = Vec::new();
    all_edges
        .try_reserve_exact(pattern.edges.len())
        .map_err(|_| ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
    all_edges.extend(&pattern.edges);
    all_edges.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    hash.update((all_edges.len() as u64).to_be_bytes());
    for edge in all_edges {
        hash.update(edge.id.canonical_bytes());
        let (start, end) = if edge.start.canonical_bytes() <= edge.end.canonical_bytes() {
            (edge.start, edge.end)
        } else {
            (edge.end, edge.start)
        };
        hash.update(start.canonical_bytes());
        hash.update(end.canonical_bytes());
        hash.update([match edge.kind {
            EdgeKind::Mountain => 0,
            EdgeKind::Valley => 1,
            EdgeKind::Auxiliary => 2,
            EdgeKind::Boundary => 3,
            EdgeKind::Cut => 4,
        }]);
    }
    hash.update((loops.len() as u64).to_be_bytes());
    for edges in &loops {
        hash.update((edges.len() as u64).to_be_bytes());
        for edge_id in edges {
            let edge = cuts
                .iter()
                .find(|edge| edge.id == *edge_id)
                .ok_or(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)?;
            let (start, end) = if edge.start.canonical_bytes() <= edge.end.canonical_bytes() {
                (edge.start, edge.end)
            } else {
                (edge.end, edge.start)
            };
            hash.update(edge_id.canonical_bytes());
            for vertex in [start, end] {
                let point = positions[&vertex];
                hash.update(vertex.canonical_bytes());
                hash.update(point.x.to_bits().to_be_bytes());
                hash.update(point.y.to_bits().to_be_bytes());
            }
        }
    }
    Ok(ClosedCutLoopDiagnosticV1 {
        loops,
        fingerprint: hash.finalize().into(),
    })
}

fn validate_intersections(
    pattern: &CreasePattern,
    positions: &HashMap<VertexId, Point2>,
    cuts: &[&ori_domain::Edge],
    max_tests: usize,
) -> Result<(), ClosedCutLoopDiagnosticErrorV1> {
    let mut tests = 0usize;
    for first in cuts {
        for second in &pattern.edges {
            if first.id == second.id
                || (second.kind == EdgeKind::Cut
                    && first.id.canonical_bytes() >= second.id.canonical_bytes())
            {
                continue;
            }
            tests = tests
                .checked_add(1)
                .ok_or(ClosedCutLoopDiagnosticErrorV1::ResourceLimit)?;
            if tests > max_tests {
                return Err(ClosedCutLoopDiagnosticErrorV1::ResourceLimit);
            }
            let a = positions[&first.start];
            let b = positions[&first.end];
            let Some(c) = positions.get(&second.start).copied() else {
                return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
            };
            let Some(d) = positions.get(&second.end).copied() else {
                return Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry);
            };
            match segment_intersection(a, b, c, d)
                .map_err(|_| ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)?
            {
                SegmentIntersection::None => {}
                SegmentIntersection::Point(point)
                    if [first.start, first.end].into_iter().any(|vertex| {
                        (vertex == second.start || vertex == second.end)
                            && positions[&vertex] == point
                    }) => {}
                SegmentIntersection::Point(_) | SegmentIntersection::CollinearOverlap => {
                    return Err(if second.kind == EdgeKind::Cut {
                        ClosedCutLoopDiagnosticErrorV1::InvalidGeometry
                    } else {
                        ClosedCutLoopDiagnosticErrorV1::MaterialEdgeIntersection
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Vertex};

    use super::*;

    fn vertex(x: f64, y: f64) -> Vertex {
        Vertex {
            id: VertexId::new(),
            position: Point2::new(x, y),
        }
    }

    fn edge(start: &Vertex, end: &Vertex, kind: EdgeKind) -> Edge {
        Edge {
            id: EdgeId::new(),
            start: start.id,
            end: end.id,
            kind,
        }
    }

    fn triangle() -> CreasePattern {
        let a = vertex(0.0, 0.0);
        let b = vertex(2.0, 0.0);
        let c = vertex(1.0, 1.0);
        let edges = vec![
            edge(&a, &b, EdgeKind::Cut),
            edge(&b, &c, EdgeKind::Cut),
            edge(&c, &a, EdgeKind::Cut),
        ];
        CreasePattern {
            vertices: vec![a, b, c],
            edges,
        }
    }

    #[test]
    fn closed_loop_is_canonical_bounded_and_nonauthoritative() {
        let pattern = triangle();
        let first =
            diagnose_closed_cut_loops_v1(&pattern, ClosedCutLoopDiagnosticLimitsV1::default())
                .unwrap();
        let mut reordered = pattern.clone();
        reordered.edges.reverse();
        for edge in &mut reordered.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        reordered.vertices.reverse();
        let second =
            diagnose_closed_cut_loops_v1(&reordered, ClosedCutLoopDiagnosticLimitsV1::default())
                .unwrap();
        assert_eq!(first.loops().len(), 1);
        assert_eq!(first.loops()[0].len(), 3);
        assert_eq!(first.fingerprint_v1(), second.fingerprint_v1());
        assert!(first.is_for(&reordered, ClosedCutLoopDiagnosticLimitsV1::default()));
        assert!(!first.authorizes_simulation_admission());
        assert!(!first.authorizes_project_mutation());
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: 2,
                    max_intersection_tests: 10,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn chain_branch_crossing_and_material_intersection_fail_closed() {
        let mut chain = triangle();
        chain.edges.pop();
        assert_eq!(
            diagnose_closed_cut_loops_v1(&chain, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop)
        );
        let mut branch = triangle();
        let center = branch.vertices[0].clone();
        let tip = vertex(-1.0, -1.0);
        branch.edges.push(edge(&center, &tip, EdgeKind::Cut));
        branch.vertices.push(tip);
        assert_eq!(
            diagnose_closed_cut_loops_v1(&branch, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop)
        );

        let a = vertex(0.0, 0.0);
        let b = vertex(2.0, 2.0);
        let c = vertex(0.0, 2.0);
        let d = vertex(2.0, 0.0);
        let crossing = CreasePattern {
            edges: vec![
                edge(&a, &b, EdgeKind::Cut),
                edge(&b, &c, EdgeKind::Cut),
                edge(&c, &d, EdgeKind::Cut),
                edge(&d, &a, EdgeKind::Cut),
            ],
            vertices: vec![a, b, c, d],
        };
        assert_eq!(
            diagnose_closed_cut_loops_v1(&crossing, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)
        );

        let mut intersected = triangle();
        let p = vertex(1.0, -1.0);
        let q = vertex(1.0, 2.0);
        intersected.edges.push(edge(&p, &q, EdgeKind::Mountain));
        intersected.vertices.extend([p, q]);
        assert_eq!(
            diagnose_closed_cut_loops_v1(&intersected, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::MaterialEdgeIntersection)
        );
    }

    #[test]
    fn geometry_tamper_changes_binding_and_work_cap_is_enforced() {
        let pattern = triangle();
        let original =
            diagnose_closed_cut_loops_v1(&pattern, ClosedCutLoopDiagnosticLimitsV1::default())
                .unwrap();
        let mut tampered = pattern.clone();
        tampered.vertices[0].position.x = -0.25;
        let changed =
            diagnose_closed_cut_loops_v1(&tampered, ClosedCutLoopDiagnosticLimitsV1::default())
                .unwrap();
        assert_ne!(original.fingerprint_v1(), changed.fingerprint_v1());
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: 3,
                    max_intersection_tests: 2,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::ResourceLimit)
        );

        let empty = CreasePattern {
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        assert_eq!(
            diagnose_closed_cut_loops_v1(&empty, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::NotClosedLoop)
        );
    }

    #[test]
    fn duplicate_coordinate_contact_and_verified_noncut_tamper_are_bound() {
        let mut pattern = triangle();
        let duplicate = vertex(0.0, 0.0);
        let p = vertex(-2.0, 0.0);
        let q = vertex(-1.0, 1.0);
        pattern.edges.extend([
            edge(&duplicate, &p, EdgeKind::Cut),
            edge(&p, &q, EdgeKind::Cut),
            edge(&q, &duplicate, EdgeKind::Cut),
        ]);
        pattern.vertices.extend([duplicate, p, q]);
        assert_eq!(
            diagnose_closed_cut_loops_v1(&pattern, ClosedCutLoopDiagnosticLimitsV1::default()),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)
        );

        let mut with_remote_edge = triangle();
        let shared = with_remote_edge.vertices[0].clone();
        let r = vertex(-10.0, -10.0);
        with_remote_edge
            .edges
            .push(edge(&shared, &r, EdgeKind::Boundary));
        with_remote_edge.vertices.push(r);
        let before = diagnose_closed_cut_loops_v1(
            &with_remote_edge,
            ClosedCutLoopDiagnosticLimitsV1::default(),
        )
        .unwrap();
        with_remote_edge.vertices[3].position.x = 12.0;
        let after = diagnose_closed_cut_loops_v1(
            &with_remote_edge,
            ClosedCutLoopDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert_ne!(before.fingerprint_v1(), after.fingerprint_v1());
    }

    #[test]
    fn every_hard_cap_and_duplicate_identity_fails_closed_at_its_boundary() {
        let pattern = triangle();
        assert!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1,
                    max_intersection_tests: 3,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            )
            .is_ok()
        );
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_vertices: MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1 + 1,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidLimit)
        );
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1 + 1,
                    max_intersection_tests: 3,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidLimit)
        );
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &pattern,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: 3,
                    max_intersection_tests: MAX_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1 + 1,
                    ..ClosedCutLoopDiagnosticLimitsV1::default()
                }
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidLimit)
        );

        let mut exact_vertices = pattern.clone();
        while exact_vertices.vertices.len() < MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1 {
            let index = exact_vertices.vertices.len() as f64;
            exact_vertices.vertices.push(vertex(index + 10.0, -10.0));
        }
        assert!(
            diagnose_closed_cut_loops_v1(
                &exact_vertices,
                ClosedCutLoopDiagnosticLimitsV1::default()
            )
            .is_ok()
        );
        let mut excessive_vertices = pattern.clone();
        let repeated = excessive_vertices.vertices[0].clone();
        excessive_vertices
            .vertices
            .resize(MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1 + 1, repeated);
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &excessive_vertices,
                ClosedCutLoopDiagnosticLimitsV1::default()
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::ResourceLimit)
        );

        let mut duplicate_vertex = pattern.clone();
        duplicate_vertex
            .vertices
            .push(duplicate_vertex.vertices[0].clone());
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &duplicate_vertex,
                ClosedCutLoopDiagnosticLimitsV1::default()
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)
        );
        let mut duplicate_edge = pattern.clone();
        duplicate_edge.edges.push(duplicate_edge.edges[0].clone());
        assert_eq!(
            diagnose_closed_cut_loops_v1(
                &duplicate_edge,
                ClosedCutLoopDiagnosticLimitsV1::default()
            ),
            Err(ClosedCutLoopDiagnosticErrorV1::InvalidGeometry)
        );
    }
}
