//! Correctness-first topology for one vertex-to-vertex fold chord.
//!
//! The caller is responsible for admitting this module only after the paper
//! and topology-participating crease records pass validation and exactly one
//! active edge has been selected. Auxiliary construction geometry is outside
//! this module's material-topology contract. This slice additionally requires
//! a mountain or valley chord between two non-adjacent boundary vertices of a
//! convex sheet.

use std::collections::HashMap;

use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, VertexId};
use ori_geometry::{Orientation, exact_orientation, polygon_signed_double_area};

use super::{BoundaryWalk, HalfEdgeRef};

/// The two material regions created by one admitted fold chord.
///
/// Both walks are counter-clockwise (material on the left) and cyclically
/// canonicalized. `left` and `right` are defined relative to the canonical
/// chord direction from the endpoint with smaller canonical ID bytes to the
/// endpoint with larger bytes. The result is therefore independent of
/// boundary orientation and starting vertex, source-record order, and the
/// source fold's endpoint direction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleFoldFaces {
    pub(crate) fold: EdgeId,
    pub(crate) canonical_start: VertexId,
    pub(crate) canonical_end: VertexId,
    pub(crate) left: BoundaryWalk,
    pub(crate) right: BoundaryWalk,
}

/// A reason the correctness-first one-fold subset could not be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleFoldError {
    /// A clockwise turn remains after normalizing the paper boundary to CCW.
    NonConvex { vertex: VertexId },
    /// The fold endpoints coincide or are neighbours on the paper boundary.
    AdjacentEndpoints {
        fold: EdgeId,
        first: VertexId,
        second: VertexId,
    },
    /// An endpoint or undirected source edge could not be resolved uniquely.
    UnresolvedEdge {
        edge: Option<EdgeId>,
        first: VertexId,
        second: VertexId,
    },
    /// One of the two closed walks has zero or unrepresentable area.
    DegenerateFace { fold: EdgeId },
}

/// Splits a validated convex paper boundary with one mountain/valley chord.
///
/// This function deliberately does not search for the fold: the integration
/// layer performs the global "exactly one active edge" admission check and
/// passes that edge explicitly. A non-fold edge is rejected as unresolved so
/// a future caller cannot accidentally turn an auxiliary or cut edge into a
/// material hinge.
pub(crate) fn extract_single_fold_faces(
    paper: &Paper,
    pattern: &CreasePattern,
    fold: &Edge,
) -> Result<SingleFoldFaces, SingleFoldError> {
    if !matches!(fold.kind, EdgeKind::Mountain | EdgeKind::Valley) {
        return Err(SingleFoldError::UnresolvedEdge {
            edge: Some(fold.id),
            first: fold.start,
            second: fold.end,
        });
    }

    let index = SourceIndex::build(pattern, fold)?;
    let mut boundary = paper.boundary_vertices.clone();
    let positions = resolve_positions(&boundary, &index, fold)?;
    let paper_area = polygon_signed_double_area(&positions)
        .map_err(|_| SingleFoldError::DegenerateFace { fold: fold.id })?;
    if paper_area == 0.0 {
        return Err(SingleFoldError::DegenerateFace { fold: fold.id });
    }
    if paper_area < 0.0 {
        boundary.reverse();
    }

    ensure_convex(&boundary, &index)?;

    let first_index = boundary
        .iter()
        .position(|vertex| *vertex == fold.start)
        .ok_or(SingleFoldError::UnresolvedEdge {
            edge: Some(fold.id),
            first: fold.start,
            second: fold.end,
        })?;
    let second_index = boundary
        .iter()
        .position(|vertex| *vertex == fold.end)
        .ok_or(SingleFoldError::UnresolvedEdge {
            edge: Some(fold.id),
            first: fold.start,
            second: fold.end,
        })?;
    let boundary_len = boundary.len();
    if first_index == second_index
        || first_index.abs_diff(second_index) == 1
        || first_index.abs_diff(second_index) == boundary_len - 1
    {
        return Err(SingleFoldError::AdjacentEndpoints {
            fold: fold.id,
            first: fold.start,
            second: fold.end,
        });
    }

    let (canonical_start, canonical_end, canonical_start_index, canonical_end_index) =
        if fold.start.canonical_bytes() <= fold.end.canonical_bytes() {
            (fold.start, fold.end, first_index, second_index)
        } else {
            (fold.end, fold.start, second_index, first_index)
        };

    // A CCW boundary walk has material on the left of every directed
    // half-edge. The walk ending its boundary chain with canonical_start ->
    // canonical_end is therefore the face left of the canonical chord.
    let left = build_walk(
        &boundary,
        canonical_end_index,
        canonical_start_index,
        fold,
        &index,
    )?;
    let right = build_walk(
        &boundary,
        canonical_start_index,
        canonical_end_index,
        fold,
        &index,
    )?;

    Ok(SingleFoldFaces {
        fold: fold.id,
        canonical_start,
        canonical_end,
        left,
        right,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UndirectedEndpoints {
    first: VertexId,
    second: VertexId,
}

impl UndirectedEndpoints {
    fn new(first: VertexId, second: VertexId) -> Self {
        if first.canonical_bytes() <= second.canonical_bytes() {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        }
    }
}

struct SourceIndex {
    positions: HashMap<VertexId, Point2>,
    boundary_edges: HashMap<UndirectedEndpoints, EdgeId>,
}

impl SourceIndex {
    fn build(pattern: &CreasePattern, fold: &Edge) -> Result<Self, SingleFoldError> {
        let mut positions = HashMap::with_capacity(pattern.vertices.len());
        for vertex in &pattern.vertices {
            if positions.insert(vertex.id, vertex.position).is_some() {
                return Err(SingleFoldError::UnresolvedEdge {
                    edge: Some(fold.id),
                    first: fold.start,
                    second: fold.end,
                });
            }
        }

        let mut boundary_edges = HashMap::new();
        for edge in pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
        {
            let endpoints = UndirectedEndpoints::new(edge.start, edge.end);
            if let Some(previous) = boundary_edges.insert(endpoints, edge.id) {
                let edge = if previous.canonical_bytes() <= edge.id.canonical_bytes() {
                    previous
                } else {
                    edge.id
                };
                return Err(SingleFoldError::UnresolvedEdge {
                    edge: Some(edge),
                    first: endpoints.first,
                    second: endpoints.second,
                });
            }
        }

        Ok(Self {
            positions,
            boundary_edges,
        })
    }

    fn position(&self, vertex: VertexId) -> Option<Point2> {
        self.positions.get(&vertex).copied()
    }

    fn boundary_edge(&self, first: VertexId, second: VertexId) -> Option<EdgeId> {
        self.boundary_edges
            .get(&UndirectedEndpoints::new(first, second))
            .copied()
    }
}

fn resolve_positions(
    vertices: &[VertexId],
    index: &SourceIndex,
    fold: &Edge,
) -> Result<Vec<Point2>, SingleFoldError> {
    vertices
        .iter()
        .map(|vertex| {
            index
                .position(*vertex)
                .ok_or(SingleFoldError::UnresolvedEdge {
                    edge: Some(fold.id),
                    first: *vertex,
                    second: *vertex,
                })
        })
        .collect()
}

fn ensure_convex(boundary: &[VertexId], index: &SourceIndex) -> Result<(), SingleFoldError> {
    let mut first_reflex = None;
    for current in 0..boundary.len() {
        let previous = boundary[(current + boundary.len() - 1) % boundary.len()];
        let vertex = boundary[current];
        let next = boundary[(current + 1) % boundary.len()];
        let [Some(previous_position), Some(position), Some(next_position)] = [
            index.position(previous),
            index.position(vertex),
            index.position(next),
        ] else {
            return Err(SingleFoldError::UnresolvedEdge {
                edge: None,
                first: previous,
                second: next,
            });
        };
        let orientation =
            exact_orientation(previous_position, position, next_position).map_err(|_| {
                SingleFoldError::UnresolvedEdge {
                    edge: None,
                    first: previous,
                    second: next,
                }
            })?;
        if orientation == Orientation::Clockwise
            && first_reflex.is_none_or(|candidate: VertexId| {
                vertex.canonical_bytes() < candidate.canonical_bytes()
            })
        {
            first_reflex = Some(vertex);
        }
    }
    first_reflex.map_or(Ok(()), |vertex| Err(SingleFoldError::NonConvex { vertex }))
}

fn build_walk(
    boundary: &[VertexId],
    start_index: usize,
    end_index: usize,
    fold: &Edge,
    index: &SourceIndex,
) -> Result<BoundaryWalk, SingleFoldError> {
    let mut half_edges = Vec::new();
    let mut cursor = start_index;
    while cursor != end_index {
        let next = (cursor + 1) % boundary.len();
        let origin = boundary[cursor];
        let destination = boundary[next];
        let edge =
            index
                .boundary_edge(origin, destination)
                .ok_or(SingleFoldError::UnresolvedEdge {
                    edge: None,
                    first: origin,
                    second: destination,
                })?;
        half_edges.push(HalfEdgeRef {
            edge,
            origin,
            destination,
        });
        cursor = next;
    }

    let chord_origin = boundary[end_index];
    let chord_destination = boundary[start_index];
    half_edges.push(HalfEdgeRef {
        edge: fold.id,
        origin: chord_origin,
        destination: chord_destination,
    });

    let positions = half_edges
        .iter()
        .map(|half_edge| index.position(half_edge.origin))
        .collect::<Option<Vec<_>>>()
        .ok_or(SingleFoldError::UnresolvedEdge {
            edge: Some(fold.id),
            first: fold.start,
            second: fold.end,
        })?;
    let signed_double_area = polygon_signed_double_area(&positions)
        .map_err(|_| SingleFoldError::DegenerateFace { fold: fold.id })?;
    if signed_double_area <= 0.0 || !signed_double_area.is_finite() {
        return Err(SingleFoldError::DegenerateFace { fold: fold.id });
    }

    canonicalize_cycle(&mut half_edges);
    Ok(BoundaryWalk {
        half_edges,
        signed_double_area,
    })
}

fn canonicalize_cycle(half_edges: &mut [HalfEdgeRef]) {
    if let Some((best, _)) = half_edges
        .iter()
        .enumerate()
        .min_by_key(|(_, half_edge)| half_edge_token(half_edge))
    {
        half_edges.rotate_left(best);
    }
}

fn half_edge_token(half_edge: &HalfEdgeRef) -> [u8; 48] {
    let mut token = [0_u8; 48];
    token[..16].copy_from_slice(&half_edge.edge.canonical_bytes());
    token[16..32].copy_from_slice(&half_edge.origin.canonical_bytes());
    token[32..].copy_from_slice(&half_edge.destination.canonical_bytes());
    token
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn square_fixture() -> (Paper, CreasePattern, Edge) {
        let vertex_ids = [
            fixed_id(0x101),
            fixed_id(0x102),
            fixed_id(0x103),
            fixed_id(0x104),
        ];
        let vertices = [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(0.0, 4.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            id: vertex_ids[index],
            position,
        })
        .collect::<Vec<_>>();
        let mut edges = (0..4)
            .map(|index| Edge {
                id: fixed_id(0x201 + index as u64),
                start: vertex_ids[(index + 1) % 4],
                end: vertex_ids[index],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let fold = Edge {
            id: fixed_id(0x301),
            start: vertex_ids[0],
            end: vertex_ids[2],
            kind: EdgeKind::Mountain,
        };
        edges.push(fold.clone());
        (
            Paper {
                boundary_vertices: vertex_ids.to_vec(),
                ..Paper::default()
            },
            CreasePattern { vertices, edges },
            fold,
        )
    }

    #[test]
    fn diagonal_produces_two_ccw_faces_with_opposite_fold_half_edges() {
        let (paper, pattern, fold) = square_fixture();

        let result = extract_single_fold_faces(&paper, &pattern, &fold).expect("split square");

        assert_eq!(result.fold, fold.id);
        assert_eq!(result.left.signed_double_area, 16.0);
        assert_eq!(result.right.signed_double_area, 16.0);
        assert!(result.canonical_start.canonical_bytes() < result.canonical_end.canonical_bytes());
        let fold_half_edges = result
            .left
            .half_edges
            .iter()
            .chain(&result.right.half_edges)
            .filter(|half_edge| half_edge.edge == fold.id)
            .collect::<Vec<_>>();
        assert_eq!(fold_half_edges.len(), 2);
        assert_eq!(fold_half_edges[0].origin, result.canonical_start);
        assert_eq!(fold_half_edges[0].destination, result.canonical_end);
        assert_eq!(fold_half_edges[1].origin, result.canonical_end);
        assert_eq!(fold_half_edges[1].destination, result.canonical_start);
    }

    #[test]
    fn result_ignores_boundary_orientation_record_order_and_edge_direction() {
        let (paper, pattern, fold) = square_fixture();
        let expected = extract_single_fold_faces(&paper, &pattern, &fold).expect("baseline");
        let mut transformed_paper = paper.clone();
        transformed_paper.boundary_vertices.reverse();
        transformed_paper.boundary_vertices.rotate_left(1);
        let mut transformed_pattern = pattern.clone();
        transformed_pattern.vertices.reverse();
        transformed_pattern.edges.reverse();
        for edge in &mut transformed_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let transformed_fold = transformed_pattern
            .edges
            .iter()
            .find(|edge| edge.id == fold.id)
            .expect("fold record");

        assert_eq!(
            extract_single_fold_faces(&transformed_paper, &transformed_pattern, transformed_fold,),
            Ok(expected)
        );
    }

    #[test]
    fn adjacent_fold_is_rejected() {
        let (paper, mut pattern, mut fold) = square_fixture();
        fold.end = paper.boundary_vertices[1];
        pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == fold.id)
            .expect("fold record")
            .end = fold.end;

        assert!(matches!(
            extract_single_fold_faces(&paper, &pattern, &fold),
            Err(SingleFoldError::AdjacentEndpoints { fold: rejected, .. }) if rejected == fold.id
        ));
    }

    #[test]
    fn concave_boundary_is_rejected_with_the_reflex_vertex() {
        let (paper, mut pattern, fold) = square_fixture();
        let reflex = paper.boundary_vertices[2];
        pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == reflex)
            .expect("reflex vertex")
            .position = Point2::new(2.0, 1.0);

        assert_eq!(
            extract_single_fold_faces(&paper, &pattern, &fold),
            Err(SingleFoldError::NonConvex { vertex: reflex })
        );
    }

    #[test]
    fn collinear_side_that_collapses_a_face_is_rejected() {
        let vertex_ids = [
            fixed_id(0x401),
            fixed_id(0x402),
            fixed_id(0x403),
            fixed_id(0x404),
        ];
        let vertices = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 2.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            id: vertex_ids[index],
            position,
        })
        .collect::<Vec<_>>();
        let mut edges = (0..4)
            .map(|index| Edge {
                id: fixed_id(0x501 + index as u64),
                start: vertex_ids[index],
                end: vertex_ids[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let fold = Edge {
            id: fixed_id(0x601),
            start: vertex_ids[0],
            end: vertex_ids[2],
            kind: EdgeKind::Valley,
        };
        edges.push(fold.clone());
        let paper = Paper {
            boundary_vertices: vertex_ids.to_vec(),
            ..Paper::default()
        };
        let pattern = CreasePattern { vertices, edges };

        assert_eq!(
            extract_single_fold_faces(&paper, &pattern, &fold),
            Err(SingleFoldError::DegenerateFace { fold: fold.id })
        );
    }
}
