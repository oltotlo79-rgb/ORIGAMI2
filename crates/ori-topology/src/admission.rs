//! Fail-closed admission for the general paper half-edge graph.
//!
//! This remains private until material-face grouping is ready for the public
//! extraction route. Keeping every prerequisite in one constructor prevents
//! the exact-midpoint containment shortcut from being used before global
//! intersection validation has proved that an active edge's open interval
//! cannot enter and leave the sheet between samples.

use std::collections::{HashMap, HashSet};

use ori_domain::{CreasePattern, EdgeId, EdgeKind, Paper, VertexId};
use ori_geometry::{
    PointPolygonRelation, segment_midpoint_polygon_relation, validate_crease_pattern,
    validate_paper,
};

use crate::dcel::{DcelBuildError, PaperWalkSet, build_paper_walks_unchecked};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaperGraphAdmissionError {
    DuplicateVertexId { vertex: VertexId },
    DuplicateEdgeId { edge: EdgeId },
    InvalidPaper { issue_count: usize },
    CutNotAllowed { edge: EdgeId },
    InvalidParticipantPattern { issue_count: usize },
    ActiveEdgeOutsidePaper { edge: EdgeId },
    ContainmentPredicateFailure { edge: EdgeId },
    ContainmentInvariantViolation { edge: EdgeId },
    InternalBoundaryResolution,
    Dcel(DcelBuildError),
}

/// A source snapshot whose paper and topology-participating edge graph have
/// passed global validation. The fields and constructor are private so the
/// midpoint containment proof cannot be detached from that prerequisite.
struct IntersectionValidatedInput<'a> {
    paper: &'a Paper,
    pattern: &'a CreasePattern,
}

/// A validated source snapshot whose non-boundary participating edges lie in
/// the closed paper polygon.
struct AdmittedPaperGraph<'a> {
    paper: &'a Paper,
    pattern: &'a CreasePattern,
}

/// Validates, admits, and embeds one general paper graph in a fixed order.
///
/// Global IDs and the paper contract are checked before policy; cut policy is
/// checked before participant geometry; participant intersections are checked
/// before exact-midpoint containment; and the DCEL is built only after all of
/// those proofs succeed.
pub(crate) fn build_admitted_paper_walks(
    paper: &Paper,
    pattern: &CreasePattern,
) -> Result<PaperWalkSet, PaperGraphAdmissionError> {
    let validated = validate_for_containment(paper, pattern)?;
    let admitted = admit_contained_graph(validated)?;
    build_paper_walks_unchecked(admitted.pattern, admitted.paper)
        .map_err(PaperGraphAdmissionError::Dcel)
}

fn validate_for_containment<'a>(
    paper: &'a Paper,
    pattern: &'a CreasePattern,
) -> Result<IntersectionValidatedInput<'a>, PaperGraphAdmissionError> {
    ensure_unique_global_ids(pattern)?;

    let paper_validation = validate_paper(paper, pattern);
    if !paper_validation.is_valid() {
        return Err(PaperGraphAdmissionError::InvalidPaper {
            issue_count: paper_validation.issues.len(),
        });
    }

    if !paper.cutting_allowed
        && let Some(edge) = pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut)
            .min_by_key(|edge| edge.id.canonical_bytes())
    {
        return Err(PaperGraphAdmissionError::CutNotAllowed { edge: edge.id });
    }

    let participants = participant_pattern(pattern);
    let participant_validation = validate_crease_pattern(&participants);
    if !participant_validation.is_valid() {
        return Err(PaperGraphAdmissionError::InvalidParticipantPattern {
            issue_count: participant_validation.issues.len(),
        });
    }

    Ok(IntersectionValidatedInput { paper, pattern })
}

fn admit_contained_graph(
    validated: IntersectionValidatedInput<'_>,
) -> Result<AdmittedPaperGraph<'_>, PaperGraphAdmissionError> {
    let positions = validated
        .pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let boundary = validated
        .paper
        .boundary_vertices
        .iter()
        .map(|vertex| {
            positions
                .get(vertex)
                .copied()
                .ok_or(PaperGraphAdmissionError::InternalBoundaryResolution)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut active_edges = validated
        .pattern
        .edges
        .iter()
        .filter(|edge| active_edge_kind(edge.kind))
        .collect::<Vec<_>>();
    active_edges.sort_by_key(|edge| edge.id.canonical_bytes());

    for edge in active_edges {
        let start = positions
            .get(&edge.start)
            .copied()
            .ok_or(PaperGraphAdmissionError::ContainmentPredicateFailure { edge: edge.id })?;
        let end = positions
            .get(&edge.end)
            .copied()
            .ok_or(PaperGraphAdmissionError::ContainmentPredicateFailure { edge: edge.id })?;
        match segment_midpoint_polygon_relation(start, end, &boundary)
            .map_err(|_| PaperGraphAdmissionError::ContainmentPredicateFailure { edge: edge.id })?
        {
            PointPolygonRelation::Outside => {
                return Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: edge.id });
            }
            PointPolygonRelation::Boundary => {
                return Err(PaperGraphAdmissionError::ContainmentInvariantViolation {
                    edge: edge.id,
                });
            }
            PointPolygonRelation::Inside => {}
        }
    }

    Ok(AdmittedPaperGraph {
        paper: validated.paper,
        pattern: validated.pattern,
    })
}

fn ensure_unique_global_ids(pattern: &CreasePattern) -> Result<(), PaperGraphAdmissionError> {
    let mut vertex_ids = HashSet::with_capacity(pattern.vertices.len());
    let duplicate_vertex = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .filter(|vertex| !vertex_ids.insert(*vertex))
        .min_by_key(VertexId::canonical_bytes);
    if let Some(vertex) = duplicate_vertex {
        return Err(PaperGraphAdmissionError::DuplicateVertexId { vertex });
    }

    let mut edge_ids = HashSet::with_capacity(pattern.edges.len());
    let duplicate_edge = pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .filter(|edge| !edge_ids.insert(*edge))
        .min_by_key(EdgeId::canonical_bytes);
    if let Some(edge) = duplicate_edge {
        return Err(PaperGraphAdmissionError::DuplicateEdgeId { edge });
    }
    Ok(())
}

fn participant_pattern(pattern: &CreasePattern) -> CreasePattern {
    let edges = pattern
        .edges
        .iter()
        .filter(|edge| topology_participant_kind(edge.kind))
        .cloned()
        .collect::<Vec<_>>();
    let vertex_ids = edges
        .iter()
        .flat_map(|edge| [edge.start, edge.end])
        .collect::<HashSet<_>>();
    let vertices = pattern
        .vertices
        .iter()
        .filter(|vertex| vertex_ids.contains(&vertex.id))
        .cloned()
        .collect();
    CreasePattern { vertices, edges }
}

fn topology_participant_kind(kind: EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Boundary | EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
    )
}

fn active_edge_kind(kind: EdgeKind) -> bool {
    matches!(kind, EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut)
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Point2, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: fixed_id(suffix),
            position: Point2::new(x, y),
        }
    }

    fn edge(suffix: u64, start: &Vertex, end: &Vertex, kind: EdgeKind) -> Edge {
        Edge {
            id: fixed_id(suffix),
            start: start.id,
            end: end.id,
            kind,
        }
    }

    fn paper(vertices: &[&Vertex], cutting_allowed: bool) -> Paper {
        Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            cutting_allowed,
            ..Paper::default()
        }
    }

    fn boundary_edges(vertices: &[&Vertex], first_suffix: u64) -> Vec<Edge> {
        (0..vertices.len())
            .map(|index| {
                edge(
                    first_suffix + index as u64,
                    vertices[index],
                    vertices[(index + 1) % vertices.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect()
    }

    #[test]
    fn boundary_only_and_internal_fold_graphs_are_admitted() {
        let a = vertex(0x101, 0.0, 0.0);
        let b = vertex(0x102, 4.0, 0.0);
        let c = vertex(0x103, 4.0, 4.0);
        let d = vertex(0x104, 0.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let mut pattern = CreasePattern {
            vertices: vec![d.clone(), b.clone(), a.clone(), c.clone()],
            edges: boundary_edges(&boundary, 0x110),
        };
        let source_paper = paper(&boundary, false);

        assert!(build_admitted_paper_walks(&source_paper, &pattern).is_ok());

        pattern.edges.push(edge(0x120, &a, &c, EdgeKind::Mountain));
        let expected = build_admitted_paper_walks(&source_paper, &pattern)
            .expect("admitted diagonal fold graph");

        let mut transformed = pattern;
        transformed.vertices.reverse();
        transformed.edges.reverse();
        for edge in &mut transformed.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut transformed_paper = source_paper;
        transformed_paper.boundary_vertices.rotate_left(1);
        transformed_paper.boundary_vertices.reverse();
        assert_eq!(
            build_admitted_paper_walks(&transformed_paper, &transformed),
            Ok(expected)
        );
    }

    #[test]
    fn concave_boundary_chord_with_outside_open_interval_is_rejected() {
        let a = vertex(0x201, 0.0, 0.0);
        let b = vertex(0x202, 4.0, 0.0);
        let c = vertex(0x203, 4.0, 4.0);
        let notch = vertex(0x204, 2.0, 2.0);
        let d = vertex(0x205, 0.0, 4.0);
        let boundary = [&a, &b, &c, &notch, &d];
        let outside = edge(0x220, &c, &d, EdgeKind::Valley);
        let mut pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), notch.clone(), d.clone()],
            edges: boundary_edges(&boundary, 0x210),
        };
        pattern.edges.push(outside.clone());

        assert_eq!(
            build_admitted_paper_walks(&paper(&boundary, false), &pattern),
            Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: outside.id })
        );

        pattern.vertices.reverse();
        pattern.edges.reverse();
        for edge in &mut pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut reversed_paper = paper(&boundary, false);
        reversed_paper.boundary_vertices.rotate_left(2);
        reversed_paper.boundary_vertices.reverse();
        assert_eq!(
            build_admitted_paper_walks(&reversed_paper, &pattern),
            Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: outside.id })
        );
    }

    #[test]
    fn canonical_first_edge_of_an_outside_component_is_reported() {
        let a = vertex(0x301, 0.0, 0.0);
        let b = vertex(0x302, 4.0, 0.0);
        let c = vertex(0x303, 4.0, 4.0);
        let d = vertex(0x304, 0.0, 4.0);
        let p = vertex(0x305, 6.0, 0.0);
        let q = vertex(0x306, 8.0, 0.0);
        let r = vertex(0x307, 7.0, 2.0);
        let boundary = [&a, &b, &c, &d];
        let first = edge(0x320, &p, &q, EdgeKind::Mountain);
        let second = edge(0x321, &q, &r, EdgeKind::Valley);
        let third = edge(0x322, &r, &p, EdgeKind::Mountain);
        let mut edges = boundary_edges(&boundary, 0x310);
        edges.extend([third, second, first.clone()]);
        let mut pattern = CreasePattern {
            vertices: vec![
                r.clone(),
                c.clone(),
                p.clone(),
                a.clone(),
                q.clone(),
                d.clone(),
                b.clone(),
            ],
            edges,
        };

        assert_eq!(
            build_admitted_paper_walks(&paper(&boundary, false), &pattern),
            Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: first.id })
        );

        pattern.vertices.reverse();
        pattern.edges.reverse();
        for edge in &mut pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut transformed_paper = paper(&boundary, false);
        transformed_paper.boundary_vertices.rotate_left(3);
        transformed_paper.boundary_vertices.reverse();
        assert_eq!(
            build_admitted_paper_walks(&transformed_paper, &pattern),
            Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: first.id })
        );
    }

    #[test]
    fn internal_mixed_loop_including_an_allowed_cut_is_admitted() {
        let a = vertex(0x401, -4.0, -4.0);
        let b = vertex(0x402, 4.0, -4.0);
        let c = vertex(0x403, 4.0, 4.0);
        let d = vertex(0x404, -4.0, 4.0);
        let p = vertex(0x405, -1.0, -1.0);
        let q = vertex(0x406, 1.0, -1.0);
        let r = vertex(0x407, 0.0, 1.0);
        let boundary = [&a, &b, &c, &d];
        let mut edges = boundary_edges(&boundary, 0x410);
        edges.extend([
            edge(0x420, &p, &q, EdgeKind::Mountain),
            edge(0x421, &q, &r, EdgeKind::Cut),
            edge(0x422, &r, &p, EdgeKind::Valley),
        ]);
        let pattern = CreasePattern {
            vertices: vec![
                r.clone(),
                d.clone(),
                a.clone(),
                q.clone(),
                c.clone(),
                p.clone(),
                b.clone(),
            ],
            edges,
        };

        assert!(build_admitted_paper_walks(&paper(&boundary, true), &pattern).is_ok());
    }

    #[test]
    fn cut_policy_precedes_participant_geometry_but_follows_paper_validation() {
        let a = vertex(0x501, 0.0, 0.0);
        let b = vertex(0x502, 4.0, 0.0);
        let c = vertex(0x503, 4.0, 4.0);
        let d = vertex(0x504, 0.0, 4.0);
        let outside = vertex(0x505, -1.0, -1.0);
        let boundary = [&a, &b, &c, &d];
        let cut = edge(0x520, &a, &outside, EdgeKind::Cut);
        let mut edges = boundary_edges(&boundary, 0x510);
        edges.push(cut.clone());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), outside],
            edges,
        };
        let source_paper = paper(&boundary, false);

        assert_eq!(
            build_admitted_paper_walks(&source_paper, &pattern),
            Err(PaperGraphAdmissionError::CutNotAllowed { edge: cut.id })
        );

        let mut cutting_paper = source_paper.clone();
        cutting_paper.cutting_allowed = true;
        assert_eq!(
            build_admitted_paper_walks(&cutting_paper, &pattern),
            Err(PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge: cut.id })
        );

        let earlier_cut = Edge {
            id: fixed_id(0x51f),
            start: a.id,
            end: fixed_id(0x506),
            kind: EdgeKind::Cut,
        };
        let mut multiple_cuts = pattern.clone();
        multiple_cuts.edges.push(earlier_cut.clone());
        assert_eq!(
            build_admitted_paper_walks(&source_paper, &multiple_cuts),
            Err(PaperGraphAdmissionError::CutNotAllowed {
                edge: earlier_cut.id
            })
        );
        assert!(matches!(
            build_admitted_paper_walks(&cutting_paper, &multiple_cuts),
            Err(PaperGraphAdmissionError::InvalidParticipantPattern { .. })
        ));

        let mut invalid_paper = source_paper;
        invalid_paper.boundary_vertices.pop();
        assert!(matches!(
            build_admitted_paper_walks(&invalid_paper, &pattern),
            Err(PaperGraphAdmissionError::InvalidPaper { .. })
        ));
    }

    #[test]
    fn participant_intersections_precede_containment() {
        let a = vertex(0x601, 0.0, 0.0);
        let b = vertex(0x602, 4.0, 0.0);
        let c = vertex(0x603, 4.0, 4.0);
        let d = vertex(0x604, 0.0, 4.0);
        let left = vertex(0x605, -1.0, 2.0);
        let right = vertex(0x606, 5.0, 2.0);
        let boundary = [&a, &b, &c, &d];
        let mut edges = boundary_edges(&boundary, 0x610);
        edges.push(edge(0x620, &left, &right, EdgeKind::Mountain));
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), left, right],
            edges,
        };

        assert!(matches!(
            build_admitted_paper_walks(&paper(&boundary, false), &pattern),
            Err(PaperGraphAdmissionError::InvalidParticipantPattern { issue_count })
                if issue_count >= 2
        ));
    }

    #[test]
    fn outside_auxiliary_geometry_is_ignored() {
        let a = vertex(0x701, 0.0, 0.0);
        let b = vertex(0x702, 4.0, 0.0);
        let c = vertex(0x703, 4.0, 4.0);
        let d = vertex(0x704, 0.0, 4.0);
        let p = vertex(0x705, f64::NAN, 1.0);
        let q = vertex(0x706, 7.0, 1.0);
        let boundary = [&a, &b, &c, &d];
        let mut edges = boundary_edges(&boundary, 0x710);
        edges.push(edge(0x720, &p, &q, EdgeKind::Auxiliary));
        let pattern = CreasePattern {
            // `q` is intentionally omitted and `p` is non-finite. Neither may
            // affect material admission because only the Auxiliary edge uses
            // them.
            vertices: vec![p, d.clone(), b.clone(), a.clone(), c.clone()],
            edges,
        };

        assert!(build_admitted_paper_walks(&paper(&boundary, false), &pattern).is_ok());
    }

    #[test]
    fn exact_subnormal_midpoint_is_admitted_inside_the_sheet() {
        let unit = f64::from_bits(1);
        let a = vertex(0x801, 0.0, 0.0);
        let b = vertex(0x802, 2.0 * unit, 0.0);
        let c = vertex(0x803, 2.0 * unit, 2.0 * unit);
        let d = vertex(0x804, 0.0, 2.0 * unit);
        let boundary_mid = vertex(0x805, 0.0, unit);
        let center = vertex(0x806, unit, unit);
        let boundary = [&a, &b, &c, &d, &boundary_mid];
        let mut edges = boundary_edges(&boundary, 0x810);
        edges.push(edge(0x820, &boundary_mid, &center, EdgeKind::Mountain));
        let pattern = CreasePattern {
            vertices: vec![
                center,
                d.clone(),
                a.clone(),
                c.clone(),
                boundary_mid.clone(),
                b.clone(),
            ],
            edges,
        };

        assert!(build_admitted_paper_walks(&paper(&boundary, false), &pattern).is_ok());
    }

    #[test]
    fn duplicate_global_ids_are_rejected_in_canonical_order() {
        let a = vertex(0x901, 0.0, 0.0);
        let b = vertex(0x902, 1.0, 0.0);
        let c = vertex(0x903, 0.0, 1.0);
        let boundary = [&a, &b, &c];
        let repeated = a.clone();
        let pattern = CreasePattern {
            vertices: vec![c.clone(), repeated, b.clone(), a.clone()],
            edges: boundary_edges(&boundary, 0x910),
        };

        assert_eq!(
            build_admitted_paper_walks(&paper(&boundary, false), &pattern),
            Err(PaperGraphAdmissionError::DuplicateVertexId { vertex: a.id })
        );

        let mut duplicated_edges = boundary_edges(&boundary, 0x910);
        let duplicate_edge = duplicated_edges[1].clone();
        duplicated_edges.insert(0, duplicate_edge.clone());
        let duplicate_edge_pattern = CreasePattern {
            vertices: vec![c.clone(), b.clone(), a.clone()],
            edges: duplicated_edges,
        };
        assert_eq!(
            build_admitted_paper_walks(&paper(&boundary, false), &duplicate_edge_pattern),
            Err(PaperGraphAdmissionError::DuplicateEdgeId {
                edge: duplicate_edge.id
            })
        );
    }

    #[test]
    fn forged_validation_token_fails_closed_on_a_boundary_midpoint() {
        let a = vertex(0xa01, 0.0, 0.0);
        let b = vertex(0xa02, 2.0, 0.0);
        let c = vertex(0xa03, 2.0, 2.0);
        let d = vertex(0xa04, 0.0, 2.0);
        let boundary = [&a, &b, &c, &d];
        let active = edge(0xa20, &a, &b, EdgeKind::Mountain);
        let mut edges = boundary_edges(&boundary, 0xa10);
        edges.push(active.clone());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone()],
            edges,
        };
        let source_paper = paper(&boundary, false);

        assert_eq!(
            admit_contained_graph(IntersectionValidatedInput {
                paper: &source_paper,
                pattern: &pattern,
            })
            .map(|_| ()),
            Err(PaperGraphAdmissionError::ContainmentInvariantViolation { edge: active.id })
        );
    }
}
