//! Deterministic half-edge embedding for topology-participating source edges.
//!
//! This module deliberately stops before material-face classification. It
//! establishes the exact rotation system, `next` relation, and canonical
//! walks that later stages will consume without changing the crate's current
//! public boundary/single-fold behavior.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use ori_domain::{CreasePattern, EdgeId, EdgeKind, Point2, VertexId};
use ori_geometry::{
    Orientation, exact_orientation, exact_polygon_orientation, polygon_signed_double_area,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct HalfEdgeIndex(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EmbeddedHalfEdge {
    pub(crate) edge: EdgeId,
    pub(crate) origin: VertexId,
    pub(crate) destination: VertexId,
    pub(crate) twin: HalfEdgeIndex,
    pub(crate) next: HalfEdgeIndex,
    origin_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VertexRotation {
    pub(crate) vertex: VertexId,
    /// Outgoing half-edges in counter-clockwise order, beginning at the
    /// positive X half-axis when one is present.
    pub(crate) outgoing: Vec<HalfEdgeIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DcelEmbedding {
    pub(crate) half_edges: Vec<EmbeddedHalfEdge>,
    /// Sorted by canonical `VertexId` bytes. Vertices without participating
    /// incident edges are intentionally absent.
    pub(crate) rotations: Vec<VertexRotation>,
    /// Exact binary64 positions for the same sorted participating vertices.
    /// Keeping these inside the embedding prevents a walk from accidentally
    /// being measured against a different crease-pattern snapshot.
    participant_vertices: Vec<EmbeddedVertexPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EmbeddedVertexPosition {
    vertex: VertexId,
    x_bits: u64,
    y_bits: u64,
}

impl EmbeddedVertexPosition {
    fn new(vertex: VertexId, position: Point2) -> Self {
        Self {
            vertex,
            x_bits: position.x.to_bits(),
            y_bits: position.y.to_bits(),
        }
    }

    fn position(self) -> Point2 {
        Point2::new(f64::from_bits(self.x_bits), f64::from_bits(self.y_bits))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CanonicalWalk {
    pub(crate) half_edges: Vec<HalfEdgeIndex>,
    /// Exact topological orientation, preserved even when the measured area
    /// rounds to signed zero.
    pub(crate) orientation: Orientation,
    /// Binary64 measurement only; never use its sign for classification.
    pub(crate) signed_double_area: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DcelBuildError {
    DuplicateVertexId {
        vertex: VertexId,
    },
    DuplicateEdgeId {
        edge: EdgeId,
    },
    MissingEndpoint {
        edge: EdgeId,
        vertex: VertexId,
    },
    NonFiniteVertex {
        vertex: VertexId,
    },
    DegenerateEdge {
        edge: EdgeId,
    },
    DuplicateEmbeddedEdge {
        first: EdgeId,
        second: EdgeId,
    },
    SameRay {
        vertex: VertexId,
        first: EdgeId,
        second: EdgeId,
    },
    PredicateFailure {
        vertex: VertexId,
    },
    AreaFailure,
    InternalInvariant,
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

#[derive(Debug, Clone, Copy)]
struct PendingHalfEdge {
    edge: EdgeId,
    origin: VertexId,
    destination: VertexId,
    twin: HalfEdgeIndex,
}

#[derive(Debug, Clone, Copy)]
struct Ray {
    half_edge: HalfEdgeIndex,
    edge: EdgeId,
    destination: Point2,
    half_plane: u8,
    token: [u8; 48],
}

/// Builds a deterministic planar rotation system from every participating
/// source edge.
///
/// `Boundary`, `Mountain`, `Valley`, and `Cut` each contribute two opposite
/// half-edges. `Auxiliary` contributes none. This constructor validates the
/// identity and local-ray assumptions it relies upon, but intentionally leaves
/// global intersection and paper-containment validation to the admission stage
/// that precedes it.
pub(crate) fn build_embedding(pattern: &CreasePattern) -> Result<DcelEmbedding, DcelBuildError> {
    let positions = index_vertices(pattern)?;
    ensure_unique_edge_ids(pattern)?;

    let mut participant_edges = pattern
        .edges
        .iter()
        .filter(|edge| participates_in_topology(edge.kind))
        .collect::<Vec<_>>();
    participant_edges.sort_by_key(|edge| edge.id.canonical_bytes());

    let mut endpoint_pairs = HashMap::with_capacity(participant_edges.len());
    let mut pending = Vec::with_capacity(participant_edges.len().saturating_mul(2));
    let mut outgoing_by_vertex: HashMap<VertexId, Vec<HalfEdgeIndex>> = HashMap::new();

    for edge in participant_edges {
        let endpoints = canonical_endpoints(edge.start, edge.end);
        let start_position = resolve_endpoint(&positions, edge.id, endpoints.first)?;
        let end_position = resolve_endpoint(&positions, edge.id, endpoints.second)?;
        if endpoints.first == endpoints.second || start_position == end_position {
            return Err(DcelBuildError::DegenerateEdge { edge: edge.id });
        }

        let endpoint_key = UndirectedEndpoints::new(endpoints.first, endpoints.second);
        if let Some(first) = endpoint_pairs.insert(endpoint_key, edge.id) {
            return Err(DcelBuildError::DuplicateEmbeddedEdge {
                first,
                second: edge.id,
            });
        }

        let forward = HalfEdgeIndex(pending.len());
        let reverse = HalfEdgeIndex(pending.len() + 1);
        pending.push(PendingHalfEdge {
            edge: edge.id,
            origin: endpoints.first,
            destination: endpoints.second,
            twin: reverse,
        });
        pending.push(PendingHalfEdge {
            edge: edge.id,
            origin: endpoints.second,
            destination: endpoints.first,
            twin: forward,
        });
        outgoing_by_vertex
            .entry(endpoints.first)
            .or_default()
            .push(forward);
        outgoing_by_vertex
            .entry(endpoints.second)
            .or_default()
            .push(reverse);
    }

    let mut vertices = outgoing_by_vertex.keys().copied().collect::<Vec<_>>();
    vertices.sort_by_key(VertexId::canonical_bytes);
    let mut rotations = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let outgoing = outgoing_by_vertex
            .remove(&vertex)
            .ok_or(DcelBuildError::InternalInvariant)?;
        rotations.push(build_rotation(vertex, outgoing, &pending, &positions)?);
    }

    let mut next = vec![None; pending.len()];
    for rotation in &rotations {
        let degree = rotation.outgoing.len();
        if degree == 0 {
            return Err(DcelBuildError::InternalInvariant);
        }
        for (position, outgoing) in rotation.outgoing.iter().copied().enumerate() {
            let incoming = pending
                .get(outgoing.0)
                .ok_or(DcelBuildError::InternalInvariant)?
                .twin;
            let clockwise = rotation.outgoing[(position + degree - 1) % degree];
            let slot = next
                .get_mut(incoming.0)
                .ok_or(DcelBuildError::InternalInvariant)?;
            if slot.replace(clockwise).is_some() {
                return Err(DcelBuildError::InternalInvariant);
            }
        }
    }

    let participant_vertices = rotations
        .iter()
        .map(|rotation| {
            let position = positions
                .get(&rotation.vertex)
                .copied()
                .ok_or(DcelBuildError::InternalInvariant)?;
            Ok(EmbeddedVertexPosition::new(rotation.vertex, position))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let participant_indices = participant_vertices
        .iter()
        .enumerate()
        .map(|(index, participant)| (participant.vertex, index))
        .collect::<HashMap<_, _>>();
    let half_edges = pending
        .into_iter()
        .enumerate()
        .map(|(index, half_edge)| {
            let next = next[index].ok_or(DcelBuildError::InternalInvariant)?;
            let origin_position = participant_indices
                .get(&half_edge.origin)
                .copied()
                .ok_or(DcelBuildError::InternalInvariant)?;
            Ok(EmbeddedHalfEdge {
                edge: half_edge.edge,
                origin: half_edge.origin,
                destination: half_edge.destination,
                twin: half_edge.twin,
                next,
                origin_position,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let embedding = DcelEmbedding {
        half_edges,
        rotations,
        participant_vertices,
    };
    verify_embedding(&embedding)?;
    Ok(embedding)
}

fn index_vertices(pattern: &CreasePattern) -> Result<HashMap<VertexId, Point2>, DcelBuildError> {
    let mut positions = HashMap::with_capacity(pattern.vertices.len());
    let mut duplicate = None;
    for vertex in &pattern.vertices {
        if positions.insert(vertex.id, vertex.position).is_some()
            && duplicate.is_none_or(|current: VertexId| {
                vertex.id.canonical_bytes() < current.canonical_bytes()
            })
        {
            duplicate = Some(vertex.id);
        }
    }
    duplicate.map_or(Ok(positions), |vertex| {
        Err(DcelBuildError::DuplicateVertexId { vertex })
    })
}

fn ensure_unique_edge_ids(pattern: &CreasePattern) -> Result<(), DcelBuildError> {
    let mut ids = HashSet::with_capacity(pattern.edges.len());
    let mut duplicate = None;
    for edge in &pattern.edges {
        if !ids.insert(edge.id)
            && duplicate
                .is_none_or(|current: EdgeId| edge.id.canonical_bytes() < current.canonical_bytes())
        {
            duplicate = Some(edge.id);
        }
    }
    duplicate.map_or(Ok(()), |edge| Err(DcelBuildError::DuplicateEdgeId { edge }))
}

fn participates_in_topology(kind: EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Boundary | EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
    )
}

fn canonical_endpoints(first: VertexId, second: VertexId) -> UndirectedEndpoints {
    UndirectedEndpoints::new(first, second)
}

fn resolve_endpoint(
    positions: &HashMap<VertexId, Point2>,
    edge: EdgeId,
    vertex: VertexId,
) -> Result<Point2, DcelBuildError> {
    let position = positions
        .get(&vertex)
        .copied()
        .ok_or(DcelBuildError::MissingEndpoint { edge, vertex })?;
    if position.x.is_finite() && position.y.is_finite() {
        Ok(position)
    } else {
        Err(DcelBuildError::NonFiniteVertex { vertex })
    }
}

fn build_rotation(
    vertex: VertexId,
    outgoing: Vec<HalfEdgeIndex>,
    pending: &[PendingHalfEdge],
    positions: &HashMap<VertexId, Point2>,
) -> Result<VertexRotation, DcelBuildError> {
    let origin = positions
        .get(&vertex)
        .copied()
        .ok_or(DcelBuildError::InternalInvariant)?;
    let mut rays = outgoing
        .into_iter()
        .map(|half_edge| {
            let half_edge_record = pending
                .get(half_edge.0)
                .ok_or(DcelBuildError::InternalInvariant)?;
            if half_edge_record.origin != vertex {
                return Err(DcelBuildError::InternalInvariant);
            }
            let destination = positions
                .get(&half_edge_record.destination)
                .copied()
                .ok_or(DcelBuildError::InternalInvariant)?;
            let half_plane =
                ray_half_plane(origin, destination).ok_or(DcelBuildError::DegenerateEdge {
                    edge: half_edge_record.edge,
                })?;
            Ok(Ray {
                half_edge,
                edge: half_edge_record.edge,
                destination,
                half_plane,
                token: half_edge_token(half_edge_record),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut predicate_failed = false;
    rays.sort_by(|left, right| {
        compare_rays(origin, left, right).unwrap_or_else(|()| {
            predicate_failed = true;
            left.token.cmp(&right.token)
        })
    });
    if predicate_failed {
        return Err(DcelBuildError::PredicateFailure { vertex });
    }

    for pair in rays.windows(2) {
        if pair[0].half_plane == pair[1].half_plane
            && exact_orientation(origin, pair[0].destination, pair[1].destination)
                .map_err(|_| DcelBuildError::PredicateFailure { vertex })?
                == Orientation::Collinear
        {
            let (first, second) = canonical_edge_pair(pair[0].edge, pair[1].edge);
            return Err(DcelBuildError::SameRay {
                vertex,
                first,
                second,
            });
        }
    }

    Ok(VertexRotation {
        vertex,
        outgoing: rays.into_iter().map(|ray| ray.half_edge).collect(),
    })
}

fn compare_rays(origin: Point2, left: &Ray, right: &Ray) -> Result<Ordering, ()> {
    let half_plane_order = left.half_plane.cmp(&right.half_plane);
    if half_plane_order != Ordering::Equal {
        return Ok(half_plane_order);
    }
    match exact_orientation(origin, left.destination, right.destination).map_err(|_| ())? {
        Orientation::CounterClockwise => Ok(Ordering::Less),
        Orientation::Clockwise => Ok(Ordering::Greater),
        // Same-ray input is rejected after sorting. A canonical fallback makes
        // the temporary ordering total without allowing it into a result.
        Orientation::Collinear => Ok(left.token.cmp(&right.token)),
    }
}

fn ray_half_plane(origin: Point2, destination: Point2) -> Option<u8> {
    if destination.y > origin.y || (destination.y == origin.y && destination.x > origin.x) {
        Some(0)
    } else if destination.y < origin.y || (destination.y == origin.y && destination.x < origin.x) {
        Some(1)
    } else {
        None
    }
}

fn canonical_edge_pair(first: EdgeId, second: EdgeId) -> (EdgeId, EdgeId) {
    if first.canonical_bytes() <= second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

fn half_edge_token(half_edge: &PendingHalfEdge) -> [u8; 48] {
    let mut token = [0_u8; 48];
    token[..16].copy_from_slice(&half_edge.edge.canonical_bytes());
    token[16..32].copy_from_slice(&half_edge.origin.canonical_bytes());
    token[32..].copy_from_slice(&half_edge.destination.canonical_bytes());
    token
}

fn embedded_half_edge_token(half_edge: &EmbeddedHalfEdge) -> [u8; 48] {
    let mut token = [0_u8; 48];
    token[..16].copy_from_slice(&half_edge.edge.canonical_bytes());
    token[16..32].copy_from_slice(&half_edge.origin.canonical_bytes());
    token[32..].copy_from_slice(&half_edge.destination.canonical_bytes());
    token
}

struct PendingCanonicalWalk {
    walk: CanonicalWalk,
    tokens: Vec<[u8; 48]>,
}

/// Enumerates every `next` cycle exactly once and returns a canonical ordering
/// that is independent of source record order and edge direction.
///
/// The embedding owns the positions used for area evaluation, so callers
/// cannot combine half-edges from one snapshot with coordinates from another.
pub(crate) fn canonical_walks(
    embedding: &DcelEmbedding,
) -> Result<Vec<CanonicalWalk>, DcelBuildError> {
    verify_embedding(embedding)?;

    const UNSEEN: u8 = 0;
    const VISITING: u8 = 1;
    const COMPLETE: u8 = 2;
    let half_edge_count = embedding.half_edges.len();
    let mut states = vec![UNSEEN; half_edge_count];
    let mut pending_walks = Vec::new();

    for start in 0..half_edge_count {
        if states[start] == COMPLETE {
            continue;
        }
        if states[start] != UNSEEN {
            return Err(DcelBuildError::InternalInvariant);
        }

        let mut indices = Vec::new();
        let mut current = start;
        loop {
            let state = states
                .get_mut(current)
                .ok_or(DcelBuildError::InternalInvariant)?;
            match *state {
                UNSEEN => {
                    *state = VISITING;
                    indices.push(HalfEdgeIndex(current));
                    if indices.len() > half_edge_count {
                        return Err(DcelBuildError::InternalInvariant);
                    }
                    current = embedding
                        .half_edges
                        .get(current)
                        .ok_or(DcelBuildError::InternalInvariant)?
                        .next
                        .0;
                }
                VISITING if current == start => break,
                // Re-entering a different point of this traversal forms a
                // lasso; entering COMPLETE merges into an earlier cycle.
                VISITING | COMPLETE => return Err(DcelBuildError::InternalInvariant),
                _ => return Err(DcelBuildError::InternalInvariant),
            }
        }

        for index in &indices {
            let state = states
                .get_mut(index.0)
                .ok_or(DcelBuildError::InternalInvariant)?;
            if *state != VISITING {
                return Err(DcelBuildError::InternalInvariant);
            }
            *state = COMPLETE;
        }
        pending_walks.push(canonicalize_and_measure_walk(embedding, indices)?);
    }

    if states.iter().any(|state| *state != COMPLETE)
        || pending_walks
            .iter()
            .map(|pending| pending.walk.half_edges.len())
            .sum::<usize>()
            != half_edge_count
    {
        return Err(DcelBuildError::InternalInvariant);
    }

    pending_walks.sort_by(|left, right| left.tokens.cmp(&right.tokens));
    Ok(pending_walks
        .into_iter()
        .map(|pending| pending.walk)
        .collect())
}

fn canonicalize_and_measure_walk(
    embedding: &DcelEmbedding,
    mut half_edges: Vec<HalfEdgeIndex>,
) -> Result<PendingCanonicalWalk, DcelBuildError> {
    if half_edges.is_empty() {
        return Err(DcelBuildError::InternalInvariant);
    }
    let mut tokens = half_edges
        .iter()
        .map(|index| {
            embedding
                .half_edges
                .get(index.0)
                .map(embedded_half_edge_token)
                .ok_or(DcelBuildError::InternalInvariant)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let minimum = tokens
        .iter()
        .enumerate()
        .min_by_key(|(_, token)| *token)
        .map(|(index, _)| index)
        .ok_or(DcelBuildError::InternalInvariant)?;
    half_edges.rotate_left(minimum);
    tokens.rotate_left(minimum);

    if tokens.iter().skip(1).any(|token| token == &tokens[0]) {
        return Err(DcelBuildError::InternalInvariant);
    }
    let positions = half_edges
        .iter()
        .map(|index| {
            let half_edge = embedding
                .half_edges
                .get(index.0)
                .ok_or(DcelBuildError::InternalInvariant)?;
            embedding
                .participant_vertices
                .get(half_edge.origin_position)
                .copied()
                .map(EmbeddedVertexPosition::position)
                .ok_or(DcelBuildError::InternalInvariant)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let orientation =
        exact_polygon_orientation(&positions).map_err(|_| DcelBuildError::AreaFailure)?;
    let signed_double_area =
        polygon_signed_double_area(&positions).map_err(|_| DcelBuildError::AreaFailure)?;
    if !signed_double_area.is_finite() {
        return Err(DcelBuildError::AreaFailure);
    }

    Ok(PendingCanonicalWalk {
        walk: CanonicalWalk {
            half_edges,
            orientation,
            signed_double_area,
        },
        tokens,
    })
}

fn verify_embedding(embedding: &DcelEmbedding) -> Result<(), DcelBuildError> {
    if embedding.participant_vertices.len() != embedding.rotations.len() {
        return Err(DcelBuildError::InternalInvariant);
    }
    for (index, participant) in embedding.participant_vertices.iter().enumerate() {
        let position = participant.position();
        if embedding.rotations[index].vertex != participant.vertex
            || !position.x.is_finite()
            || !position.y.is_finite()
            || index > 0
                && embedding.participant_vertices[index - 1]
                    .vertex
                    .canonical_bytes()
                    >= participant.vertex.canonical_bytes()
        {
            return Err(DcelBuildError::InternalInvariant);
        }
    }

    let mut seen_outgoing = vec![false; embedding.half_edges.len()];
    for rotation in &embedding.rotations {
        if rotation.outgoing.is_empty() {
            return Err(DcelBuildError::InternalInvariant);
        }
        for half_edge in &rotation.outgoing {
            let record = embedding
                .half_edges
                .get(half_edge.0)
                .ok_or(DcelBuildError::InternalInvariant)?;
            if record.origin != rotation.vertex || seen_outgoing[half_edge.0] {
                return Err(DcelBuildError::InternalInvariant);
            }
            seen_outgoing[half_edge.0] = true;
        }
    }
    if seen_outgoing.iter().any(|seen| !seen) {
        return Err(DcelBuildError::InternalInvariant);
    }

    let mut seen_next = vec![false; embedding.half_edges.len()];
    let mut seen_tokens = HashSet::with_capacity(embedding.half_edges.len());
    for (index, half_edge) in embedding.half_edges.iter().enumerate() {
        let twin = embedding
            .half_edges
            .get(half_edge.twin.0)
            .ok_or(DcelBuildError::InternalInvariant)?;
        let next = embedding
            .half_edges
            .get(half_edge.next.0)
            .ok_or(DcelBuildError::InternalInvariant)?;
        let origin_position = embedding
            .participant_vertices
            .get(half_edge.origin_position)
            .ok_or(DcelBuildError::InternalInvariant)?;
        if twin.twin != HalfEdgeIndex(index)
            || twin.edge != half_edge.edge
            || twin.origin != half_edge.destination
            || twin.destination != half_edge.origin
            || next.origin != half_edge.destination
            || origin_position.vertex != half_edge.origin
            || seen_next[half_edge.next.0]
            || !seen_tokens.insert(embedded_half_edge_token(half_edge))
        {
            return Err(DcelBuildError::InternalInvariant);
        }
        seen_next[half_edge.next.0] = true;
    }
    if seen_next.iter().any(|seen| !seen) {
        return Err(DcelBuildError::InternalInvariant);
    }
    Ok(())
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

    fn assert_total_invariants(embedding: &DcelEmbedding, participant_edges: usize) {
        assert_eq!(embedding.half_edges.len(), participant_edges * 2);
        assert_eq!(verify_embedding(embedding), Ok(()));
        assert!(
            embedding
                .half_edges
                .iter()
                .enumerate()
                .all(|(index, half_edge)| {
                    embedding.half_edges[half_edge.twin.0].twin == HalfEdgeIndex(index)
                        && half_edge.next.0 < embedding.half_edges.len()
                })
        );
    }

    fn assert_walk_invariants(embedding: &DcelEmbedding, walks: &[CanonicalWalk]) {
        assert_eq!(
            walks
                .iter()
                .map(|walk| walk.half_edges.len())
                .sum::<usize>(),
            embedding.half_edges.len()
        );
        let mut seen = vec![false; embedding.half_edges.len()];
        let token_sequences = walks
            .iter()
            .map(|walk| {
                assert!(!walk.half_edges.is_empty());
                let tokens = walk
                    .half_edges
                    .iter()
                    .map(|index| {
                        assert!(!std::mem::replace(&mut seen[index.0], true));
                        embedded_half_edge_token(&embedding.half_edges[index.0])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(tokens[0], *tokens.iter().min().expect("minimum token"));
                for position in 0..walk.half_edges.len() {
                    let current = walk.half_edges[position];
                    let following = walk.half_edges[(position + 1) % walk.half_edges.len()];
                    assert_eq!(embedding.half_edges[current.0].next, following);
                }
                tokens
            })
            .collect::<Vec<_>>();
        assert!(seen.into_iter().all(|was_seen| was_seen));
        assert!(token_sequences.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn outgoing_destinations(embedding: &DcelEmbedding, vertex: VertexId) -> Vec<VertexId> {
        let rotation = embedding
            .rotations
            .iter()
            .find(|rotation| rotation.vertex == vertex)
            .expect("vertex rotation");
        rotation
            .outgoing
            .iter()
            .map(|index| embedding.half_edges[index.0].destination)
            .collect()
    }

    fn half_edge(
        embedding: &DcelEmbedding,
        origin: VertexId,
        destination: VertexId,
    ) -> HalfEdgeIndex {
        HalfEdgeIndex(
            embedding
                .half_edges
                .iter()
                .position(|half_edge| {
                    half_edge.origin == origin && half_edge.destination == destination
                })
                .expect("directed half-edge"),
        )
    }

    #[test]
    fn square_has_canonical_twins_and_left_face_next_links() {
        let a = vertex(0x101, 0.0, 0.0);
        let b = vertex(0x102, 4.0, 0.0);
        let c = vertex(0x103, 4.0, 4.0);
        let d = vertex(0x104, 0.0, 4.0);
        let pattern = CreasePattern {
            vertices: vec![d.clone(), b.clone(), a.clone(), c.clone()],
            edges: vec![
                edge(0x204, &a, &d, EdgeKind::Boundary),
                edge(0x202, &c, &b, EdgeKind::Boundary),
                edge(0x201, &a, &b, EdgeKind::Boundary),
                edge(0x203, &c, &d, EdgeKind::Boundary),
            ],
        };

        let embedding = build_embedding(&pattern).expect("square embedding");

        assert_total_invariants(&embedding, 4);
        for pair in embedding.half_edges.chunks_exact(2) {
            assert!(pair[0].edge.canonical_bytes() <= pair[1].edge.canonical_bytes());
            assert!(pair[0].origin.canonical_bytes() < pair[0].destination.canonical_bytes());
            assert_eq!(pair[0].origin, pair[1].destination);
            assert_eq!(pair[0].destination, pair[1].origin);
        }
        let a_to_b = half_edge(&embedding, a.id, b.id);
        let b_to_a = half_edge(&embedding, b.id, a.id);
        assert_eq!(
            embedding.half_edges[a_to_b.0].next,
            half_edge(&embedding, b.id, c.id)
        );
        assert_eq!(
            embedding.half_edges[b_to_a.0].next,
            half_edge(&embedding, a.id, d.id)
        );

        let walks = canonical_walks(&embedding).expect("square walks");
        assert_walk_invariants(&embedding, &walks);
        let mut areas = walks
            .iter()
            .map(|walk| walk.signed_double_area)
            .collect::<Vec<_>>();
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas, vec![-32.0, 32.0]);
        assert_eq!(
            walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::CounterClockwise)
                .count(),
            1
        );
        assert_eq!(
            walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::Clockwise)
                .count(),
            1
        );
    }

    #[test]
    fn degree_three_t_rotation_is_exactly_counter_clockwise() {
        let center = vertex(0x110, 0.0, 0.0);
        let east = vertex(0x111, 2.0, 0.0);
        let north = vertex(0x112, 0.0, 2.0);
        let west = vertex(0x113, -2.0, 0.0);
        let auxiliary = Edge {
            id: fixed_id(0x999),
            start: fixed_id(0xdead),
            end: fixed_id(0xbeef),
            kind: EdgeKind::Auxiliary,
        };
        let pattern = CreasePattern {
            vertices: vec![north.clone(), center.clone(), west.clone(), east.clone()],
            edges: vec![
                edge(0x303, &center, &west, EdgeKind::Cut),
                auxiliary,
                edge(0x301, &east, &center, EdgeKind::Mountain),
                edge(0x302, &center, &north, EdgeKind::Valley),
            ],
        };

        let embedding = build_embedding(&pattern).expect("degree-three embedding");

        assert_total_invariants(&embedding, 3);
        assert_eq!(
            outgoing_destinations(&embedding, center.id),
            vec![east.id, north.id, west.id]
        );
        let walks = canonical_walks(&embedding).expect("tree walk");
        assert_walk_invariants(&embedding, &walks);
        assert_eq!(walks.len(), 1);
        assert_eq!(walks[0].half_edges.len(), 6);
        assert_eq!(walks[0].signed_double_area, 0.0);
        assert_eq!(walks[0].orientation, Orientation::Collinear);
    }

    #[test]
    fn disconnected_parallel_edges_produce_two_zero_area_walks() {
        let lower_left = vertex(0x180, 0.0, 0.0);
        let lower_right = vertex(0x181, 1.0, 0.0);
        let upper_left = vertex(0x182, 0.0, 2.0);
        let upper_right = vertex(0x183, 1.0, 2.0);
        let pattern = CreasePattern {
            vertices: vec![
                upper_right.clone(),
                lower_left.clone(),
                upper_left.clone(),
                lower_right.clone(),
            ],
            edges: vec![
                edge(0xa02, &upper_right, &upper_left, EdgeKind::Cut),
                edge(0xa01, &lower_left, &lower_right, EdgeKind::Mountain),
            ],
        };

        let embedding = build_embedding(&pattern).expect("disconnected embedding");
        let walks = canonical_walks(&embedding).expect("disconnected walks");

        assert_walk_invariants(&embedding, &walks);
        assert_eq!(walks.len(), 2);
        assert!(walks.iter().all(|walk| {
            walk.half_edges.len() == 2
                && walk.signed_double_area == 0.0
                && walk.orientation == Orientation::Collinear
        }));
    }

    #[test]
    fn exact_walk_orientation_survives_binary64_area_underflow() {
        let origin = vertex(0x190, 0.0, 0.0);
        let east = vertex(0x191, f64::MIN_POSITIVE, 0.0);
        let north = vertex(0x192, 0.0, f64::MIN_POSITIVE);
        let pattern = CreasePattern {
            vertices: vec![north.clone(), origin.clone(), east.clone()],
            edges: vec![
                edge(0xb03, &north, &origin, EdgeKind::Boundary),
                edge(0xb01, &origin, &east, EdgeKind::Boundary),
                edge(0xb02, &east, &north, EdgeKind::Boundary),
            ],
        };

        let embedding = build_embedding(&pattern).expect("underflow triangle embedding");
        let walks = canonical_walks(&embedding).expect("underflow triangle walks");

        assert_walk_invariants(&embedding, &walks);
        assert_eq!(walks.len(), 2);
        assert!(walks.iter().all(|walk| walk.signed_double_area == 0.0));
        assert_eq!(
            walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::CounterClockwise)
                .count(),
            1
        );
        assert_eq!(
            walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::Clockwise)
                .count(),
            1
        );
    }

    #[test]
    fn degree_four_x_rotation_ignores_record_order_ids_and_edge_directions() {
        let center = vertex(0x120, 0.0, 0.0);
        let north_east = vertex(0x124, 1.0, 1.0);
        let north_west = vertex(0x123, -1.0, 1.0);
        let south_west = vertex(0x122, -1.0, -1.0);
        let south_east = vertex(0x121, 1.0, -1.0);
        let vertices = vec![
            center.clone(),
            north_east.clone(),
            north_west.clone(),
            south_west.clone(),
            south_east.clone(),
        ];
        let edges = vec![
            edge(0x404, &south_east, &center, EdgeKind::Valley),
            edge(0x401, &center, &north_east, EdgeKind::Mountain),
            edge(0x403, &south_west, &center, EdgeKind::Cut),
            edge(0x402, &north_west, &center, EdgeKind::Boundary),
        ];
        let pattern = CreasePattern {
            vertices: vertices.clone(),
            edges: edges.clone(),
        };
        let mut transformed_vertices = vertices;
        transformed_vertices.reverse();
        let mut transformed_edges = edges;
        transformed_edges.reverse();
        for edge in &mut transformed_edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let transformed = CreasePattern {
            vertices: transformed_vertices,
            edges: transformed_edges,
        };

        let expected = build_embedding(&pattern).expect("degree-four embedding");
        let actual = build_embedding(&transformed).expect("transformed embedding");

        assert_total_invariants(&expected, 4);
        assert_eq!(actual, expected);
        assert_eq!(
            outgoing_destinations(&expected, center.id),
            vec![north_east.id, north_west.id, south_west.id, south_east.id]
        );
    }

    #[test]
    fn cardinal_rotation_handles_extreme_coordinates_and_uses_clockwise_predecessor() {
        let center = vertex(0x130, -f64::MAX / 2.0, 0.0);
        let east = vertex(0x131, f64::MAX, 0.0);
        let north = vertex(0x132, -f64::MAX / 2.0, f64::MAX);
        let west = vertex(0x133, -f64::MAX, 0.0);
        let south = vertex(0x134, -f64::MAX / 2.0, -f64::MAX);
        assert!((east.position.x - center.position.x).is_infinite());
        let pattern = CreasePattern {
            vertices: vec![
                west.clone(),
                center.clone(),
                south.clone(),
                east.clone(),
                north.clone(),
            ],
            edges: vec![
                edge(0x504, &south, &center, EdgeKind::Cut),
                edge(0x502, &north, &center, EdgeKind::Valley),
                edge(0x503, &center, &west, EdgeKind::Boundary),
                edge(0x501, &east, &center, EdgeKind::Mountain),
            ],
        };

        let embedding = build_embedding(&pattern).expect("extreme cardinal embedding");

        assert_total_invariants(&embedding, 4);
        assert_eq!(
            outgoing_destinations(&embedding, center.id),
            vec![east.id, north.id, west.id, south.id]
        );
        let west_to_center = half_edge(&embedding, west.id, center.id);
        assert_eq!(
            embedding.half_edges[west_to_center.0].next,
            half_edge(&embedding, center.id, north.id)
        );
    }

    #[test]
    fn split_square_walks_are_canonical_across_storage_kind_and_auxiliary_changes() {
        let south_west = vertex(0x160, -2.0, -2.0);
        let south_east = vertex(0x161, 2.0, -2.0);
        let north_east = vertex(0x162, 2.0, 2.0);
        let north_west = vertex(0x163, -2.0, 2.0);
        let center = vertex(0x164, 0.0, 0.0);
        let vertices = vec![
            south_west.clone(),
            south_east.clone(),
            north_east.clone(),
            north_west.clone(),
            center.clone(),
        ];
        let edges = vec![
            edge(0x801, &south_west, &south_east, EdgeKind::Boundary),
            edge(0x802, &south_east, &north_east, EdgeKind::Boundary),
            edge(0x803, &north_east, &north_west, EdgeKind::Boundary),
            edge(0x804, &north_west, &south_west, EdgeKind::Boundary),
            edge(0x805, &center, &south_west, EdgeKind::Mountain),
            edge(0x806, &center, &south_east, EdgeKind::Mountain),
            edge(0x807, &center, &north_east, EdgeKind::Mountain),
            edge(0x808, &center, &north_west, EdgeKind::Mountain),
        ];
        let baseline = CreasePattern {
            vertices: vertices.clone(),
            edges: edges.clone(),
        };

        let mut transformed_vertices = vertices;
        transformed_vertices.reverse();
        let mut transformed_edges = edges;
        transformed_edges.reverse();
        for edge in &mut transformed_edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
            if edge.kind == EdgeKind::Mountain {
                edge.kind = EdgeKind::Cut;
            }
        }
        transformed_edges.push(Edge {
            id: fixed_id(0x8ff),
            start: fixed_id(0xcafe),
            end: fixed_id(0xbabe),
            kind: EdgeKind::Auxiliary,
        });
        let transformed = CreasePattern {
            vertices: transformed_vertices,
            edges: transformed_edges,
        };

        let baseline_embedding = build_embedding(&baseline).expect("split-square embedding");
        let transformed_embedding =
            build_embedding(&transformed).expect("transformed split-square embedding");
        let baseline_walks = canonical_walks(&baseline_embedding).expect("split-square walks");
        let transformed_walks =
            canonical_walks(&transformed_embedding).expect("transformed split-square walks");

        assert_eq!(transformed_embedding, baseline_embedding);
        assert_eq!(transformed_walks, baseline_walks);
        assert_walk_invariants(&baseline_embedding, &baseline_walks);
        assert_eq!(baseline_walks.len(), 5);
        let mut areas = baseline_walks
            .iter()
            .map(|walk| walk.signed_double_area)
            .collect::<Vec<_>>();
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas, vec![-32.0, 8.0, 8.0, 8.0, 8.0]);
        assert_eq!(
            baseline_walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::CounterClockwise)
                .count(),
            4
        );
        assert_eq!(
            baseline_walks
                .iter()
                .filter(|walk| walk.orientation == Orientation::Clockwise)
                .count(),
            1
        );
    }

    #[test]
    fn walk_enumeration_fails_closed_on_invalid_next_and_area_overflow() {
        let a = vertex(0x170, -f64::MAX, -f64::MAX);
        let b = vertex(0x171, f64::MAX, -f64::MAX);
        let c = vertex(0x172, f64::MAX, f64::MAX);
        let d = vertex(0x173, -f64::MAX, f64::MAX);
        let huge = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone()],
            edges: vec![
                edge(0x901, &a, &b, EdgeKind::Boundary),
                edge(0x902, &b, &c, EdgeKind::Boundary),
                edge(0x903, &c, &d, EdgeKind::Boundary),
                edge(0x904, &d, &a, EdgeKind::Boundary),
            ],
        };
        let embedding = build_embedding(&huge).expect("finite extreme embedding");
        assert_eq!(
            canonical_walks(&embedding),
            Err(DcelBuildError::AreaFailure)
        );

        let mut invalid_index = embedding.clone();
        invalid_index.half_edges[0].next = HalfEdgeIndex(invalid_index.half_edges.len());
        assert_eq!(
            canonical_walks(&invalid_index),
            Err(DcelBuildError::InternalInvariant)
        );

        let mut merged_cycle = embedding;
        merged_cycle.half_edges[0].next = merged_cycle.half_edges[1].next;
        assert_eq!(
            canonical_walks(&merged_cycle),
            Err(DcelBuildError::InternalInvariant)
        );
    }

    #[test]
    fn exact_rotation_resolves_a_determinant_that_rounds_to_zero() {
        let center = vertex(0x140, 0.0, 0.0);
        let epsilon = f64::EPSILON;
        let clockwise = vertex(0x141, 1.0 + epsilon, 1.0);
        let counter_clockwise = vertex(0x142, 1.0 + 2.0 * epsilon, 1.0 + epsilon);
        let rounded_determinant = clockwise.position.x * counter_clockwise.position.y
            - clockwise.position.y * counter_clockwise.position.x;
        assert_eq!(rounded_determinant, 0.0);
        assert_eq!(
            exact_orientation(
                center.position,
                clockwise.position,
                counter_clockwise.position,
            ),
            Ok(Orientation::CounterClockwise)
        );
        let pattern = CreasePattern {
            vertices: vec![counter_clockwise.clone(), center.clone(), clockwise.clone()],
            // Reverse the edge-ID order so an unrelated stable-ID fallback
            // would produce the wrong geometric rotation.
            edges: vec![
                edge(0x702, &center, &clockwise, EdgeKind::Mountain),
                edge(0x701, &center, &counter_clockwise, EdgeKind::Valley),
            ],
        };

        let embedding = build_embedding(&pattern).expect("exact cancellation embedding");

        assert_total_invariants(&embedding, 2);
        assert_eq!(
            outgoing_destinations(&embedding, center.id),
            vec![clockwise.id, counter_clockwise.id]
        );
    }

    #[test]
    fn unresolved_duplicate_and_same_ray_inputs_fail_closed() {
        let center = vertex(0x501, 0.0, 0.0);
        let near = vertex(0x502, 1.0, 0.0);
        let far = vertex(0x503, 2.0, 0.0);
        let missing: VertexId = fixed_id(0x5ff);
        let first = edge(0x601, &center, &near, EdgeKind::Mountain);
        let second = edge(0x602, &center, &far, EdgeKind::Valley);

        let same_ray = build_embedding(&CreasePattern {
            vertices: vec![center.clone(), near.clone(), far.clone()],
            edges: vec![second.clone(), first.clone()],
        });
        assert_eq!(
            same_ray,
            Err(DcelBuildError::SameRay {
                vertex: center.id,
                first: first.id,
                second: second.id,
            })
        );

        let mut unresolved = first.clone();
        unresolved.end = missing;
        assert_eq!(
            build_embedding(&CreasePattern {
                vertices: vec![center.clone(), near.clone()],
                edges: vec![unresolved.clone()],
            }),
            Err(DcelBuildError::MissingEndpoint {
                edge: unresolved.id,
                vertex: missing,
            })
        );

        let mut duplicate_id = second.clone();
        duplicate_id.id = first.id;
        assert_eq!(
            build_embedding(&CreasePattern {
                vertices: vec![center.clone(), near.clone(), far.clone()],
                edges: vec![first.clone(), duplicate_id],
            }),
            Err(DcelBuildError::DuplicateEdgeId { edge: first.id })
        );

        let mut duplicate_pair = first.clone();
        duplicate_pair.id = fixed_id(0x603);
        std::mem::swap(&mut duplicate_pair.start, &mut duplicate_pair.end);
        assert_eq!(
            build_embedding(&CreasePattern {
                vertices: vec![center, near],
                edges: vec![duplicate_pair.clone(), first.clone()],
            }),
            Err(DcelBuildError::DuplicateEmbeddedEdge {
                first: first.id,
                second: duplicate_pair.id,
            })
        );
    }
}
