//! Material faces and hinge incidence for an admitted fold graph.
//!
//! This first general-graph slice supports boundary, mountain, valley, and
//! ignored auxiliary records. Every active crease must separate two distinct
//! simple material walks, and every material face must have one counter-
//! clockwise boundary component. Closed disconnected crease loops and cuts
//! remain explicit errors until holes and material separation are represented
//! in the public snapshot contract. Admission errors deliberately precede
//! this module's capability errors; therefore a forbidden or malformed cut is
//! rejected by admission before an admitted, allowed cut reaches
//! [`FoldGraphError::UnsupportedCut`].

use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, VertexId};
use ori_geometry::{Orientation, PointPolygonRelation, point_polygon_relation};

use crate::{
    BoundaryWalk, CooperativeAnalysisCheckpoint, CooperativeOperationError, EdgeIncidence, Face,
    FaceAdjacency, FaceExtractionInput, FoldAssignment, HalfEdgeRef, MaterialComponent,
    TopologyIssueKind, TopologySnapshot,
    admission::{PaperGraphAdmissionError, build_admitted_paper_walks_with_checkpoint},
    connected_sheet_component,
    dcel::{HalfEdgeIndex, PaperWalkSet, WalkIndex},
    face_from_walk, poll_cooperative_checkpoint, run_cooperative_checkpoint,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoldGraphError {
    Admission(PaperGraphAdmissionError),
    DisconnectedParticipantGraph { edge: EdgeId },
    UnexpectedWalkOrientation { edge: EdgeId },
    NonSeparatingFold { edge: EdgeId },
    ExteriorFoldIncidence { edge: EdgeId },
    UnsupportedNonSimpleFace { edge: EdgeId },
    FaceBuild(TopologyIssueKind),
    InternalInvariant,
}

#[derive(Debug, Clone, Copy)]
struct FoldSides {
    edge: EdgeId,
    left: WalkIndex,
    right: WalkIndex,
    assignment: FoldAssignment,
}

#[derive(Debug, Clone, Copy)]
struct ParticipantCounts {
    vertices: usize,
    edges: usize,
    components: usize,
    boundary_edges: usize,
    interior_edges: usize,
}

struct IncidenceResult {
    edges: Vec<(EdgeId, EdgeIncidence)>,
    hinges: Vec<FaceAdjacency>,
}

/// Extracts one deterministic snapshot from an admitted, cut-free fold graph.
#[cfg(test)]
pub(crate) fn extract_fold_graph_snapshot(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, FoldGraphError> {
    let mut checkpoint = || CooperativeAnalysisCheckpoint::Continue;
    match extract_fold_graph_snapshot_with_checkpoint(input, &mut checkpoint) {
        Ok(snapshot) => Ok(snapshot),
        Err(CooperativeOperationError::Operation(error)) => Err(error),
        Err(CooperativeOperationError::Aborted(_)) => {
            unreachable!("the no-op fold-graph checkpoint cannot abort")
        }
    }
}

pub(crate) fn extract_fold_graph_snapshot_with_checkpoint<F>(
    input: FaceExtractionInput<'_>,
    checkpoint: &mut F,
) -> Result<TopologySnapshot, CooperativeOperationError<FoldGraphError>>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    let walks = build_admitted_paper_walks_with_checkpoint(input.paper, input.pattern, checkpoint)
        .map_err(|error| error.map_operation(FoldGraphError::Admission))?;

    run_cooperative_checkpoint(checkpoint)?;
    let counts = classify_participant_components(input.paper, input.pattern)
        .map_err(CooperativeOperationError::Operation)?;
    run_cooperative_checkpoint(checkpoint)?;
    ensure_euler_partition(&walks, counts).map_err(CooperativeOperationError::Operation)?;
    let halves_by_edge = index_half_edges(&walks).map_err(CooperativeOperationError::Operation)?;
    run_cooperative_checkpoint(checkpoint)?;
    let fold_sides = resolve_fold_sides(input.pattern.edges.as_slice(), &walks, &halves_by_edge)
        .map_err(CooperativeOperationError::Operation)?;
    run_cooperative_checkpoint(checkpoint)?;
    ensure_supported_walk_orientations(&walks).map_err(CooperativeOperationError::Operation)?;
    ensure_simple_material_walks(&walks).map_err(CooperativeOperationError::Operation)?;

    let (mut faces, faces_by_walk) = build_faces(input.identity_namespace, &walks, checkpoint)?;
    let IncidenceResult {
        edges: mut edge_incidence,
        hinges: mut hinge_adjacency,
    } = build_incidence(
        input.pattern.edges.as_slice(),
        &walks,
        &halves_by_edge,
        &fold_sides,
        &faces_by_walk,
    )
    .map_err(CooperativeOperationError::Operation)?;

    run_cooperative_checkpoint(checkpoint)?;
    faces.sort_by_key(|face| face.key);
    edge_incidence.sort_by_key(|(edge, _)| edge.canonical_bytes());
    sort_hinge_adjacency(&mut hinge_adjacency, &faces)
        .map_err(CooperativeOperationError::Operation)?;
    let material_components =
        build_material_components(input.identity_namespace, &faces, &hinge_adjacency)
            .map_err(CooperativeOperationError::Operation)?;
    run_cooperative_checkpoint(checkpoint)?;

    Ok(TopologySnapshot {
        source_revision: input.source_revision,
        material_components,
        faces,
        edge_incidence,
        hinge_adjacency,
    })
}

fn build_material_components(
    sheet_origin: ori_domain::ProjectId,
    faces: &[Face],
    hinges: &[FaceAdjacency],
) -> Result<Vec<MaterialComponent>, FoldGraphError> {
    let faces_by_id = faces
        .iter()
        .map(|face| (face.id, face))
        .collect::<HashMap<_, _>>();
    if faces_by_id.len() != faces.len() {
        return Err(FoldGraphError::InternalInvariant);
    }
    let mut neighbours: HashMap<FaceId, Vec<FaceId>> = HashMap::new();
    for hinge in hinges {
        if !faces_by_id.contains_key(&hinge.first) || !faces_by_id.contains_key(&hinge.second) {
            return Err(FoldGraphError::InternalInvariant);
        }
        neighbours
            .entry(hinge.first)
            .or_default()
            .push(hinge.second);
        neighbours
            .entry(hinge.second)
            .or_default()
            .push(hinge.first);
    }
    let mut ordered_faces = faces.iter().collect::<Vec<_>>();
    ordered_faces.sort_by_key(|face| face.key);
    let mut visited = HashSet::new();
    let mut components = Vec::new();
    for root in ordered_faces {
        if !visited.insert(root.id) {
            continue;
        }
        let mut pending = vec![root.id];
        let mut component_faces = Vec::new();
        while let Some(face_id) = pending.pop() {
            let face = faces_by_id
                .get(&face_id)
                .copied()
                .ok_or(FoldGraphError::InternalInvariant)?;
            component_faces.push(face.clone());
            for neighbour in neighbours.get(&face_id).into_iter().flatten() {
                if visited.insert(*neighbour) {
                    pending.push(*neighbour);
                }
            }
        }
        components.push(connected_sheet_component(sheet_origin, &component_faces));
    }
    components.sort_by_key(|component| component.key);
    Ok(components)
}

fn sort_hinge_adjacency(
    adjacency: &mut Vec<FaceAdjacency>,
    faces: &[Face],
) -> Result<(), FoldGraphError> {
    let keys_by_id = faces
        .iter()
        .map(|face| (face.id, face.key))
        .collect::<HashMap<_, _>>();
    if keys_by_id.len() != faces.len() {
        return Err(FoldGraphError::InternalInvariant);
    }
    let mut keyed = adjacency
        .drain(..)
        .map(|adjacency| {
            let first = keys_by_id
                .get(&adjacency.first)
                .copied()
                .ok_or(FoldGraphError::InternalInvariant)?;
            let second = keys_by_id
                .get(&adjacency.second)
                .copied()
                .ok_or(FoldGraphError::InternalInvariant)?;
            if first >= second {
                return Err(FoldGraphError::InternalInvariant);
            }
            Ok((first, second, adjacency.edge.canonical_bytes(), adjacency))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by_key(|(first, second, edge, _)| (*first, *second, *edge));
    adjacency.extend(keyed.into_iter().map(|(_, _, _, adjacency)| adjacency));
    Ok(())
}

fn classify_participant_components(
    paper: &Paper,
    pattern: &CreasePattern,
) -> Result<ParticipantCounts, FoldGraphError> {
    let participant_edges = pattern
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind,
                EdgeKind::Boundary | EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
            )
        })
        .collect::<Vec<_>>();
    let mut adjacency: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
    let mut participant_vertices = HashSet::new();
    for edge in &participant_edges {
        participant_vertices.extend([edge.start, edge.end]);
        adjacency.entry(edge.start).or_default().push(edge.end);
        adjacency.entry(edge.end).or_default().push(edge.start);
    }

    let root = paper
        .boundary_vertices
        .first()
        .copied()
        .ok_or(FoldGraphError::InternalInvariant)?;
    let mut reached = HashSet::with_capacity(participant_vertices.len());
    let mut components = 0usize;
    let mut roots = Vec::with_capacity(participant_vertices.len());
    roots.push(root);
    roots.extend(
        participant_vertices
            .iter()
            .copied()
            .filter(|vertex| *vertex != root),
    );
    roots[1..].sort_by_key(VertexId::canonical_bytes);
    for component_root in roots {
        if reached.contains(&component_root) {
            continue;
        }
        components = components
            .checked_add(1)
            .ok_or(FoldGraphError::InternalInvariant)?;
        let mut component_vertices = HashSet::new();
        let mut queue = VecDeque::from([component_root]);
        while let Some(vertex) = queue.pop_front() {
            if !reached.insert(vertex) {
                continue;
            }
            component_vertices.insert(vertex);
            let neighbours = adjacency
                .get(&vertex)
                .ok_or(FoldGraphError::InternalInvariant)?;
            queue.extend(
                neighbours
                    .iter()
                    .copied()
                    .filter(|neighbour| !reached.contains(neighbour)),
            );
        }
        if component_root != root
            && let Some(edge) = participant_edges
                .iter()
                .filter(|edge| {
                    component_vertices.contains(&edge.start)
                        && matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                })
                .map(|edge| edge.id)
                .min_by_key(EdgeId::canonical_bytes)
        {
            return Err(FoldGraphError::DisconnectedParticipantGraph { edge });
        }
    }

    Ok(ParticipantCounts {
        vertices: participant_vertices.len(),
        edges: participant_edges.len(),
        components,
        boundary_edges: participant_edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        interior_edges: participant_edges
            .iter()
            .filter(|edge| {
                matches!(
                    edge.kind,
                    EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
                )
            })
            .count(),
    })
}

fn ensure_euler_partition(
    walks: &PaperWalkSet,
    counts: ParticipantCounts,
) -> Result<(), FoldGraphError> {
    let expected_walks = counts
        .edges
        .checked_add(
            counts
                .components
                .checked_mul(2)
                .ok_or(FoldGraphError::InternalInvariant)?,
        )
        .and_then(|sum| sum.checked_sub(counts.vertices))
        .ok_or(FoldGraphError::InternalInvariant)?;
    let expected_half_edges = counts
        .edges
        .checked_mul(2)
        .ok_or(FoldGraphError::InternalInvariant)?;
    if walks.walks().len() != expected_walks || walks.half_edges().len() != expected_half_edges {
        return Err(FoldGraphError::InternalInvariant);
    }

    let material_occurrences = walks
        .walks()
        .iter()
        .enumerate()
        .filter(|(index, _)| WalkIndex(*index) != walks.exterior())
        .map(|(_, walk)| walk.half_edges.len())
        .sum::<usize>();
    let expected_material_occurrences = counts
        .interior_edges
        .checked_mul(2)
        .and_then(|fold_occurrences| fold_occurrences.checked_add(counts.boundary_edges))
        .ok_or(FoldGraphError::InternalInvariant)?;
    if material_occurrences != expected_material_occurrences {
        return Err(FoldGraphError::InternalInvariant);
    }
    Ok(())
}

fn index_half_edges(
    walks: &PaperWalkSet,
) -> Result<HashMap<EdgeId, [HalfEdgeIndex; 2]>, FoldGraphError> {
    let mut pending: HashMap<EdgeId, Vec<HalfEdgeIndex>> = HashMap::new();
    for (index, half_edge) in walks.half_edges().iter().enumerate() {
        pending
            .entry(half_edge.edge)
            .or_default()
            .push(HalfEdgeIndex(index));
    }

    let mut indexed = HashMap::with_capacity(pending.len());
    for (edge, halves) in pending {
        let halves: [HalfEdgeIndex; 2] = halves
            .try_into()
            .map_err(|_| FoldGraphError::InternalInvariant)?;
        if indexed.insert(edge, halves).is_some() {
            return Err(FoldGraphError::InternalInvariant);
        }
    }
    Ok(indexed)
}

fn resolve_fold_sides(
    source_edges: &[Edge],
    walks: &PaperWalkSet,
    halves_by_edge: &HashMap<EdgeId, [HalfEdgeIndex; 2]>,
) -> Result<Vec<FoldSides>, FoldGraphError> {
    let mut folds = source_edges
        .iter()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .collect::<Vec<_>>();
    folds.sort_by_key(|edge| edge.id.canonical_bytes());

    folds
        .into_iter()
        .map(|edge| {
            let halves = halves_by_edge
                .get(&edge.id)
                .copied()
                .ok_or(FoldGraphError::InternalInvariant)?;
            let (canonical_start, canonical_end) = canonical_endpoints(edge.start, edge.end);
            let left_half = halves
                .into_iter()
                .find(|index| {
                    walks.half_edges().get(index.0).is_some_and(|half_edge| {
                        half_edge.kind == edge.kind
                            && half_edge.origin == canonical_start
                            && half_edge.destination == canonical_end
                    })
                })
                .ok_or(FoldGraphError::InternalInvariant)?;
            let right_half = walks
                .half_edges()
                .get(left_half.0)
                .map(|half_edge| half_edge.twin)
                .ok_or(FoldGraphError::InternalInvariant)?;
            if !halves.contains(&right_half) {
                return Err(FoldGraphError::InternalInvariant);
            }

            let left = walks
                .walk_owner(left_half)
                .ok_or(FoldGraphError::InternalInvariant)?;
            let right = walks
                .walk_owner(right_half)
                .ok_or(FoldGraphError::InternalInvariant)?;
            if left == walks.exterior() || right == walks.exterior() {
                return Err(FoldGraphError::ExteriorFoldIncidence { edge: edge.id });
            }
            if left == right {
                return Err(FoldGraphError::NonSeparatingFold { edge: edge.id });
            }

            let assignment = match edge.kind {
                EdgeKind::Mountain => FoldAssignment::Mountain,
                EdgeKind::Valley => FoldAssignment::Valley,
                _ => return Err(FoldGraphError::InternalInvariant),
            };
            Ok(FoldSides {
                edge: edge.id,
                left,
                right,
                assignment,
            })
        })
        .collect()
}

fn ensure_supported_walk_orientations(walks: &PaperWalkSet) -> Result<(), FoldGraphError> {
    let mut unsupported = None;
    for (walk_index, walk) in walks.walks().iter().enumerate() {
        if WalkIndex(walk_index) == walks.exterior()
            || matches!(
                walk.orientation,
                Orientation::CounterClockwise | Orientation::Clockwise | Orientation::Collinear
            )
        {
            continue;
        }
        unsupported = minimum_edge(
            unsupported,
            minimum_active_edge(walks, WalkIndex(walk_index)),
        );
    }
    if let Some(edge) = unsupported {
        Err(FoldGraphError::UnexpectedWalkOrientation { edge })
    } else if walks.walks().iter().enumerate().any(|(index, walk)| {
        WalkIndex(index) != walks.exterior()
            && !matches!(
                walk.orientation,
                Orientation::CounterClockwise | Orientation::Clockwise | Orientation::Collinear
            )
    }) {
        Err(FoldGraphError::InternalInvariant)
    } else {
        Ok(())
    }
}

fn ensure_simple_material_walks(walks: &PaperWalkSet) -> Result<(), FoldGraphError> {
    let mut unsupported = None;
    let mut found_repeated_vertex = false;
    for (walk_index, walk) in walks.walks().iter().enumerate() {
        let walk_index = WalkIndex(walk_index);
        if walk_index == walks.exterior() || walk.orientation != Orientation::CounterClockwise {
            continue;
        }
        let mut vertices = HashSet::with_capacity(walk.half_edges.len());
        let mut repeated = false;
        for half_edge in &walk.half_edges {
            let record = walks
                .half_edges()
                .get(half_edge.0)
                .ok_or(FoldGraphError::InternalInvariant)?;
            if record.kind != EdgeKind::Cut {
                repeated |= !vertices.insert(record.origin);
            }
        }
        if repeated {
            found_repeated_vertex = true;
            unsupported = minimum_edge(unsupported, minimum_active_edge(walks, walk_index));
        }
    }
    if let Some(edge) = unsupported {
        Err(FoldGraphError::UnsupportedNonSimpleFace { edge })
    } else if found_repeated_vertex {
        Err(FoldGraphError::InternalInvariant)
    } else {
        Ok(())
    }
}

fn minimum_active_edge(walks: &PaperWalkSet, walk: WalkIndex) -> Option<EdgeId> {
    walks
        .walks()
        .get(walk.0)?
        .half_edges
        .iter()
        .filter_map(|index| walks.half_edges().get(index.0))
        .filter(|half_edge| matches!(half_edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .map(|half_edge| half_edge.edge)
        .min_by_key(EdgeId::canonical_bytes)
}

fn minimum_edge(current: Option<EdgeId>, candidate: Option<EdgeId>) -> Option<EdgeId> {
    current
        .into_iter()
        .chain(candidate)
        .min_by_key(EdgeId::canonical_bytes)
}

type BuiltFaces = (Vec<Face>, HashMap<WalkIndex, Face>);

fn build_faces<F>(
    identity_namespace: ori_domain::ProjectId,
    walks: &PaperWalkSet,
    checkpoint: &mut F,
) -> Result<BuiltFaces, CooperativeOperationError<FoldGraphError>>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    let mut faces = Vec::with_capacity(walks.walks().len().saturating_sub(1));
    let mut owners = HashMap::with_capacity(faces.capacity());
    let mut keys = HashSet::with_capacity(faces.capacity());
    let mut ids = HashSet::with_capacity(faces.capacity());

    for (walk_index, walk) in walks.walks().iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, walk_index)?;
        let walk_index = WalkIndex(walk_index);
        if walk_index == walks.exterior() || walk.orientation != Orientation::CounterClockwise {
            continue;
        }
        let records = walk
            .half_edges
            .iter()
            .map(|index| {
                walks
                    .half_edges()
                    .get(index.0)
                    .map(|half_edge| {
                        (
                            half_edge.kind,
                            HalfEdgeRef {
                                edge: half_edge.edge,
                                origin: half_edge.origin,
                                destination: half_edge.destination,
                            },
                        )
                    })
                    .ok_or({
                        CooperativeOperationError::Operation(FoldGraphError::InternalInvariant)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut cut_occurrences = HashMap::<EdgeId, usize>::new();
        for (kind, half_edge) in &records {
            if *kind == EdgeKind::Cut {
                *cut_occurrences.entry(half_edge.edge).or_default() += 1;
            }
        }
        let half_edges = records
            .iter()
            .filter(|(kind, half_edge)| {
                *kind != EdgeKind::Cut || cut_occurrences.get(&half_edge.edge).copied() != Some(2)
            })
            .map(|(_, half_edge)| *half_edge)
            .collect::<Vec<_>>();
        let seam_half_edges = records
            .iter()
            .filter(|(kind, half_edge)| {
                *kind == EdgeKind::Cut && cut_occurrences.get(&half_edge.edge).copied() == Some(2)
            })
            .map(|(_, half_edge)| *half_edge)
            .collect::<Vec<_>>();
        let face = face_from_walk(
            identity_namespace,
            BoundaryWalk {
                half_edges,
                signed_double_area: walk.signed_double_area,
            },
        )
        .map_err(|error| CooperativeOperationError::Operation(FoldGraphError::FaceBuild(error)))?;
        let mut face = face;
        if !seam_half_edges.is_empty() {
            face.seams.push(BoundaryWalk {
                half_edges: seam_half_edges,
                signed_double_area: 0.0,
            });
        }
        if !keys.insert(face.key) || !ids.insert(face.id) {
            return Err(CooperativeOperationError::Operation(
                FoldGraphError::InternalInvariant,
            ));
        }
        if owners.insert(walk_index, faces.len()).is_some() {
            return Err(CooperativeOperationError::Operation(
                FoldGraphError::InternalInvariant,
            ));
        }
        faces.push(face);
    }
    for (walk_index, walk) in walks.walks().iter().enumerate() {
        let walk_index = WalkIndex(walk_index);
        if walk_index == walks.exterior()
            || !matches!(
                walk.orientation,
                Orientation::Clockwise | Orientation::Collinear
            )
        {
            continue;
        }
        let boundary =
            boundary_walk(walks, walk_index).map_err(CooperativeOperationError::Operation)?;
        let sample = boundary
            .half_edges
            .first()
            .and_then(|half_edge| walks.vertex_position(half_edge.origin))
            .ok_or(CooperativeOperationError::Operation(
                FoldGraphError::InternalInvariant,
            ))?;
        let mut owner = None;
        for (candidate_index, candidate) in faces.iter().enumerate() {
            let polygon = candidate
                .outer
                .half_edges
                .iter()
                .map(|half_edge| {
                    walks
                        .vertex_position(half_edge.origin)
                        .ok_or(FoldGraphError::InternalInvariant)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(CooperativeOperationError::Operation)?;
            if point_polygon_relation(sample, &polygon).map_err(|_| {
                CooperativeOperationError::Operation(FoldGraphError::InternalInvariant)
            })? == PointPolygonRelation::Inside
                && owner.is_none_or(|current: usize| faces[current].area > candidate.area)
            {
                owner = Some(candidate_index);
            }
        }
        let owner = owner.ok_or(CooperativeOperationError::Operation(
            FoldGraphError::InternalInvariant,
        ))?;
        if walk.orientation == Orientation::Clockwise {
            faces[owner].area -= boundary.signed_double_area.abs() * 0.5;
            faces[owner].holes.push(boundary);
        } else {
            faces[owner].seams.push(boundary);
        }
        owners.insert(walk_index, owner);
    }
    let by_walk = owners
        .into_iter()
        .map(|(walk, owner)| {
            faces.get(owner).cloned().map(|face| (walk, face)).ok_or(
                CooperativeOperationError::Operation(FoldGraphError::InternalInvariant),
            )
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    run_cooperative_checkpoint(checkpoint)?;
    Ok((faces, by_walk))
}

fn boundary_walk(walks: &PaperWalkSet, walk: WalkIndex) -> Result<BoundaryWalk, FoldGraphError> {
    let source = walks
        .walks()
        .get(walk.0)
        .ok_or(FoldGraphError::InternalInvariant)?;
    let half_edges = source
        .half_edges
        .iter()
        .map(|index| {
            walks
                .half_edges()
                .get(index.0)
                .map(|half_edge| HalfEdgeRef {
                    edge: half_edge.edge,
                    origin: half_edge.origin,
                    destination: half_edge.destination,
                })
                .ok_or(FoldGraphError::InternalInvariant)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BoundaryWalk {
        half_edges,
        signed_double_area: source.signed_double_area,
    })
}

fn build_incidence(
    source_edges: &[Edge],
    walks: &PaperWalkSet,
    halves_by_edge: &HashMap<EdgeId, [HalfEdgeIndex; 2]>,
    fold_sides: &[FoldSides],
    faces_by_walk: &HashMap<WalkIndex, Face>,
) -> Result<IncidenceResult, FoldGraphError> {
    let sides_by_edge = fold_sides
        .iter()
        .map(|sides| (sides.edge, *sides))
        .collect::<HashMap<_, _>>();
    if sides_by_edge.len() != fold_sides.len() {
        return Err(FoldGraphError::InternalInvariant);
    }

    let mut edge_incidence = Vec::with_capacity(source_edges.len());
    for edge in source_edges {
        let incidence = match edge.kind {
            EdgeKind::Boundary => {
                let halves = halves_by_edge
                    .get(&edge.id)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let owners = halves
                    .iter()
                    .map(|half_edge| {
                        walks
                            .walk_owner(*half_edge)
                            .ok_or(FoldGraphError::InternalInvariant)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let material_walk = match owners.as_slice() {
                    [first, second]
                        if *first == walks.exterior() && *second != walks.exterior() =>
                    {
                        *second
                    }
                    [first, second]
                        if *second == walks.exterior() && *first != walks.exterior() =>
                    {
                        *first
                    }
                    _ => return Err(FoldGraphError::InternalInvariant),
                };
                let material = faces_by_walk
                    .get(&material_walk)
                    .map(|face| face.id)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                EdgeIncidence::Boundary { material }
            }
            EdgeKind::Mountain | EdgeKind::Valley => {
                let sides = sides_by_edge
                    .get(&edge.id)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let left = face_id(faces_by_walk, sides.left)?;
                let right = face_id(faces_by_walk, sides.right)?;
                if left == right {
                    return Err(FoldGraphError::InternalInvariant);
                }
                EdgeIncidence::Hinge {
                    left,
                    right,
                    assignment: sides.assignment,
                }
            }
            EdgeKind::Auxiliary => EdgeIncidence::AuxiliaryIgnored,
            EdgeKind::Cut => {
                let halves = halves_by_edge
                    .get(&edge.id)
                    .copied()
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let (canonical_start, canonical_end) = canonical_endpoints(edge.start, edge.end);
                let left_half = halves
                    .into_iter()
                    .find(|index| {
                        walks.half_edges().get(index.0).is_some_and(|half_edge| {
                            half_edge.origin == canonical_start
                                && half_edge.destination == canonical_end
                        })
                    })
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let right_half = walks
                    .half_edges()
                    .get(left_half.0)
                    .map(|half_edge| half_edge.twin)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let left_walk = walks
                    .walk_owner(left_half)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                let right_walk = walks
                    .walk_owner(right_half)
                    .ok_or(FoldGraphError::InternalInvariant)?;
                if left_walk == walks.exterior() || right_walk == walks.exterior() {
                    return Err(FoldGraphError::ExteriorFoldIncidence { edge: edge.id });
                }
                EdgeIncidence::Cut {
                    left: face_id(faces_by_walk, left_walk)?,
                    right: face_id(faces_by_walk, right_walk)?,
                }
            }
        };
        edge_incidence.push((edge.id, incidence));
    }

    let mut hinge_adjacency = Vec::with_capacity(fold_sides.len());
    for sides in fold_sides {
        let left = faces_by_walk
            .get(&sides.left)
            .ok_or(FoldGraphError::InternalInvariant)?;
        let right = faces_by_walk
            .get(&sides.right)
            .ok_or(FoldGraphError::InternalInvariant)?;
        let (first, second) = if left.key < right.key {
            (left.id, right.id)
        } else if right.key < left.key {
            (right.id, left.id)
        } else {
            return Err(FoldGraphError::InternalInvariant);
        };
        hinge_adjacency.push(FaceAdjacency {
            edge: sides.edge,
            first,
            second,
            assignment: sides.assignment,
        });
    }
    Ok(IncidenceResult {
        edges: edge_incidence,
        hinges: hinge_adjacency,
    })
}

fn face_id(
    faces_by_walk: &HashMap<WalkIndex, Face>,
    walk: WalkIndex,
) -> Result<FaceId, FoldGraphError> {
    faces_by_walk
        .get(&walk)
        .map(|face| face.id)
        .ok_or(FoldGraphError::InternalInvariant)
}

fn canonical_endpoints(first: VertexId, second: VertexId) -> (VertexId, VertexId) {
    if first.canonical_bytes() <= second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, Paper, Point2, ProjectId, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;
    use crate::analyze_faces;

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

    fn input<'a>(
        namespace: ProjectId,
        paper: &'a Paper,
        pattern: &'a CreasePattern,
    ) -> FaceExtractionInput<'a> {
        FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 73,
            paper,
            pattern,
        }
    }

    fn split_x_fixture() -> (ProjectId, Paper, CreasePattern, [EdgeId; 4]) {
        let namespace = fixed_id(1);
        let a = vertex(0x101, 0.0, 0.0);
        let b = vertex(0x102, 4.0, 0.0);
        let c = vertex(0x103, 4.0, 4.0);
        let d = vertex(0x104, 0.0, 4.0);
        let center = vertex(0x105, 2.0, 2.0);
        let boundary = [&a, &b, &c, &d];
        let folds = [
            edge(0x301, &a, &center, EdgeKind::Mountain),
            edge(0x302, &center, &c, EdgeKind::Mountain),
            edge(0x303, &b, &center, EdgeKind::Valley),
            edge(0x304, &center, &d, EdgeKind::Valley),
        ];
        let fold_ids = folds.each_ref().map(|edge| edge.id);
        let mut edges = boundary_edges(&boundary, 0x201);
        edges.extend(folds.into_iter().rev());
        let pattern = CreasePattern {
            vertices: vec![center, d.clone(), b.clone(), a.clone(), c.clone()],
            edges,
        };
        (namespace, paper(&boundary, false), pattern, fold_ids)
    }

    #[test]
    fn split_x_becomes_four_faces_and_four_real_hinges() {
        let (namespace, source_paper, pattern, folds) = split_x_fixture();
        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("split X fold graph");

        assert_eq!(snapshot.faces.len(), 4);
        assert_eq!(snapshot.edge_incidence.len(), 8);
        assert_eq!(snapshot.hinge_adjacency.len(), 4);
        assert_eq!(
            snapshot.faces.iter().map(|face| face.area).sum::<f64>(),
            16.0
        );
        assert!(snapshot.faces.iter().all(|face| face.area == 4.0));
        assert!(
            snapshot
                .faces
                .windows(2)
                .all(|faces| faces[0].key < faces[1].key)
        );
        assert!(
            snapshot
                .edge_incidence
                .windows(2)
                .all(|edges| edges[0].0.canonical_bytes() < edges[1].0.canonical_bytes())
        );
        let keys_by_id = snapshot
            .faces
            .iter()
            .map(|face| (face.id, face.key))
            .collect::<HashMap<_, _>>();
        let adjacency_keys = snapshot
            .hinge_adjacency
            .iter()
            .map(|adjacency| {
                (
                    keys_by_id[&adjacency.first],
                    keys_by_id[&adjacency.second],
                    adjacency.edge.canonical_bytes(),
                )
            })
            .collect::<Vec<_>>();
        assert!(adjacency_keys.windows(2).all(|pair| pair[0] < pair[1]));

        for fold in folds {
            assert!(matches!(
                snapshot
                    .edge_incidence
                    .iter()
                    .find(|(edge, _)| *edge == fold)
                    .map(|(_, incidence)| incidence),
                Some(EdgeIncidence::Hinge { left, right, .. }) if left != right
            ));
            assert!(
                snapshot
                    .hinge_adjacency
                    .iter()
                    .any(|hinge| hinge.edge == fold)
            );
        }
    }

    #[test]
    fn split_x_snapshot_is_invariant_under_all_storage_transforms() {
        let (namespace, source_paper, pattern, _) = split_x_fixture();
        let expected = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("baseline split X");
        let mut transformed_pattern = pattern;
        transformed_pattern.vertices.reverse();
        transformed_pattern.edges.reverse();
        for edge in &mut transformed_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut transformed_paper = source_paper;
        transformed_paper.boundary_vertices.rotate_left(2);
        transformed_paper.boundary_vertices.reverse();

        assert_eq!(
            extract_fold_graph_snapshot(input(namespace, &transformed_paper, &transformed_pattern)),
            Ok(expected)
        );
    }

    #[test]
    fn assignment_changes_leave_faces_and_identity_unchanged() {
        let (namespace, source_paper, pattern, _) = split_x_fixture();
        let baseline = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("baseline assignments");
        let mut reversed_assignments = pattern;
        for edge in &mut reversed_assignments.edges {
            edge.kind = match edge.kind {
                EdgeKind::Mountain => EdgeKind::Valley,
                EdgeKind::Valley => EdgeKind::Mountain,
                kind => kind,
            };
        }
        let changed =
            extract_fold_graph_snapshot(input(namespace, &source_paper, &reversed_assignments))
                .expect("reversed assignments");

        assert_eq!(changed.faces, baseline.faces);
        assert_eq!(changed.edge_incidence.len(), baseline.edge_incidence.len());
        for ((changed_edge, changed), (baseline_edge, baseline)) in
            changed.edge_incidence.iter().zip(&baseline.edge_incidence)
        {
            assert_eq!(changed_edge, baseline_edge);
            match (changed, baseline) {
                (
                    EdgeIncidence::Hinge {
                        left: changed_left,
                        right: changed_right,
                        assignment: changed_assignment,
                    },
                    EdgeIncidence::Hinge {
                        left: baseline_left,
                        right: baseline_right,
                        assignment: baseline_assignment,
                    },
                ) => {
                    assert_eq!(
                        (changed_left, changed_right),
                        (baseline_left, baseline_right)
                    );
                    assert_ne!(changed_assignment, baseline_assignment);
                }
                _ => assert_eq!(changed, baseline),
            }
        }
        for (changed, baseline) in changed
            .hinge_adjacency
            .iter()
            .zip(&baseline.hinge_adjacency)
        {
            assert_eq!(changed.edge, baseline.edge);
            assert_eq!(
                (changed.first, changed.second),
                (baseline.first, baseline.second)
            );
            assert_ne!(changed.assignment, baseline.assignment);
        }
    }

    #[test]
    fn two_parallel_chords_become_three_cellular_faces() {
        let namespace = fixed_id(1);
        let a = vertex(0x801, 0.0, 0.0);
        let b = vertex(0x802, 2.0, 0.0);
        let c = vertex(0x803, 4.0, 0.0);
        let d = vertex(0x804, 6.0, 0.0);
        let e = vertex(0x805, 6.0, 4.0);
        let f = vertex(0x806, 4.0, 4.0);
        let g = vertex(0x807, 2.0, 4.0);
        let h = vertex(0x808, 0.0, 4.0);
        let boundary = [&a, &b, &c, &d, &e, &f, &g, &h];
        let first = edge(0x820, &b, &g, EdgeKind::Mountain);
        let second = edge(0x821, &c, &f, EdgeKind::Valley);
        let mut edges = boundary_edges(&boundary, 0x810);
        edges.extend([second.clone(), first.clone()]);
        let pattern = CreasePattern {
            vertices: vec![
                h.clone(),
                f.clone(),
                d.clone(),
                b.clone(),
                a.clone(),
                c.clone(),
                e.clone(),
                g.clone(),
            ],
            edges,
        };
        let source_paper = paper(&boundary, false);
        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("two cellular chords");

        assert_eq!(snapshot.faces.len(), 3);
        assert_eq!(snapshot.hinge_adjacency.len(), 2);
        assert!(snapshot.faces.iter().all(|face| face.area == 8.0));
        let first_faces = hinge_faces(&snapshot, first.id);
        let second_faces = hinge_faces(&snapshot, second.id);
        assert_eq!(
            first_faces
                .into_iter()
                .filter(|face| second_faces.contains(face))
                .count(),
            1,
            "the middle panel is incident to both chords"
        );
    }

    #[test]
    fn one_fold_general_graph_matches_the_existing_public_snapshot_exactly() {
        let namespace = fixed_id(1);
        let a = vertex(0x401, 0.0, 0.0);
        let b = vertex(0x402, 4.0, 0.0);
        let c = vertex(0x403, 4.0, 4.0);
        let d = vertex(0x404, 0.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let mut edges = boundary_edges(&boundary, 0x410);
        edges.push(edge(0x420, &a, &c, EdgeKind::Mountain));
        let pattern = CreasePattern {
            vertices: vec![d.clone(), b.clone(), a.clone(), c.clone()],
            edges,
        };
        let source_paper = paper(&boundary, false);
        let extraction_input = input(namespace, &source_paper, &pattern);
        let legacy = analyze_faces(extraction_input)
            .snapshot
            .expect("legacy single-fold snapshot");

        assert_eq!(extract_fold_graph_snapshot(extraction_input), Ok(legacy));
    }

    #[test]
    fn boundary_only_general_graph_matches_the_existing_public_snapshot_exactly() {
        let namespace = fixed_id(1);
        let a = vertex(0x451, 0.0, 0.0);
        let b = vertex(0x452, 4.0, 0.0);
        let c = vertex(0x453, 4.0, 4.0);
        let d = vertex(0x454, 0.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let pattern = CreasePattern {
            vertices: vec![c.clone(), a.clone(), d.clone(), b.clone()],
            edges: boundary_edges(&boundary, 0x460),
        };
        let source_paper = paper(&boundary, false);
        let extraction_input = input(namespace, &source_paper, &pattern);
        let legacy = analyze_faces(extraction_input)
            .snapshot
            .expect("legacy boundary-only snapshot");

        assert_eq!(extract_fold_graph_snapshot(extraction_input), Ok(legacy));
    }

    #[test]
    fn dangling_fold_is_rejected_as_non_separating() {
        let namespace = fixed_id(1);
        let a = vertex(0x501, 0.0, 0.0);
        let b = vertex(0x502, 4.0, 0.0);
        let c = vertex(0x503, 4.0, 4.0);
        let d = vertex(0x504, 0.0, 4.0);
        let center = vertex(0x505, 2.0, 2.0);
        let boundary = [&a, &b, &c, &d];
        let fold = edge(0x520, &a, &center, EdgeKind::Valley);
        let mut edges = boundary_edges(&boundary, 0x510);
        edges.push(fold.clone());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), center],
            edges,
        };
        let source_paper = paper(&boundary, false);

        assert_eq!(
            extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern)),
            Err(FoldGraphError::NonSeparatingFold { edge: fold.id })
        );
    }

    #[test]
    fn disconnected_closed_fold_loop_is_explicitly_rejected() {
        let namespace = fixed_id(1);
        let a = vertex(0x601, 0.0, 0.0);
        let b = vertex(0x602, 6.0, 0.0);
        let c = vertex(0x603, 6.0, 6.0);
        let d = vertex(0x604, 0.0, 6.0);
        let p = vertex(0x605, 2.0, 2.0);
        let q = vertex(0x606, 4.0, 2.0);
        let r = vertex(0x607, 3.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let first = edge(0x620, &p, &q, EdgeKind::Mountain);
        let mut edges = boundary_edges(&boundary, 0x610);
        edges.extend([
            edge(0x622, &r, &p, EdgeKind::Valley),
            edge(0x621, &q, &r, EdgeKind::Mountain),
            first.clone(),
        ]);
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), p, q, r],
            edges,
        };
        let source_paper = paper(&boundary, false);

        assert_eq!(
            extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern)),
            Err(FoldGraphError::DisconnectedParticipantGraph { edge: first.id })
        );
    }

    #[test]
    fn allowed_cut_splits_material_without_creating_a_hinge() {
        let namespace = fixed_id(1);
        let a = vertex(0x701, 0.0, 0.0);
        let b = vertex(0x702, 4.0, 0.0);
        let c = vertex(0x703, 4.0, 4.0);
        let d = vertex(0x704, 0.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let cut = edge(0x720, &a, &c, EdgeKind::Cut);
        let mut edges = boundary_edges(&boundary, 0x710);
        edges.push(cut.clone());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone()],
            edges,
        };
        let source_paper = paper(&boundary, true);

        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("allowed cut snapshot");
        assert_eq!(snapshot.faces.len(), 2);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert_eq!(snapshot.material_components.len(), 2);
        assert!(snapshot.material_components.iter().all(|component| {
            component.sheet_origin == namespace && component.faces.len() == 1
        }));
        assert!(matches!(
            snapshot
                .edge_incidence
                .iter()
                .find(|(edge, _)| *edge == cut.id)
                .map(|(_, incidence)| incidence),
            Some(EdgeIncidence::Cut { left, right }) if left != right
        ));

        let forbidden_paper = paper(&boundary, false);
        assert_eq!(
            extract_fold_graph_snapshot(input(namespace, &forbidden_paper, &pattern)),
            Err(FoldGraphError::Admission(
                PaperGraphAdmissionError::CutNotAllowed { edge: cut.id }
            ))
        );
    }

    #[test]
    fn closed_cut_loop_creates_an_inner_piece_and_an_outer_face_with_a_hole() {
        let namespace = fixed_id(1);
        let a = vertex(0x801, 0.0, 0.0);
        let b = vertex(0x802, 8.0, 0.0);
        let c = vertex(0x803, 8.0, 8.0);
        let d = vertex(0x804, 0.0, 8.0);
        let p = vertex(0x805, 2.0, 2.0);
        let q = vertex(0x806, 6.0, 2.0);
        let r = vertex(0x807, 4.0, 6.0);
        let boundary = [&a, &b, &c, &d];
        let cuts = [
            edge(0x820, &p, &q, EdgeKind::Cut),
            edge(0x821, &q, &r, EdgeKind::Cut),
            edge(0x822, &r, &p, EdgeKind::Cut),
        ];
        let mut edges = boundary_edges(&boundary, 0x810);
        edges.extend(cuts.iter().cloned());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), p, q, r],
            edges,
        };
        let source_paper = paper(&boundary, true);

        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("closed cut loop");
        assert_eq!(snapshot.faces.len(), 2);
        assert_eq!(snapshot.material_components.len(), 2);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert_eq!(
            snapshot
                .faces
                .iter()
                .filter(|face| face.holes.len() == 1)
                .count(),
            1
        );
        assert!(snapshot.faces.iter().all(|face| face.seams.is_empty()));
        assert!(cuts.iter().all(|cut| {
            matches!(
                snapshot
                    .edge_incidence
                    .iter()
                    .find(|(edge, _)| *edge == cut.id)
                    .map(|(_, incidence)| incidence),
                Some(EdgeIncidence::Cut { left, right }) if left != right
            )
        }));
    }

    #[test]
    fn isolated_branched_cut_is_one_open_seam_without_disconnect() {
        let namespace = fixed_id(1);
        let a = vertex(0x901, 0.0, 0.0);
        let b = vertex(0x902, 8.0, 0.0);
        let c = vertex(0x903, 8.0, 8.0);
        let d = vertex(0x904, 0.0, 8.0);
        let center = vertex(0x905, 4.0, 4.0);
        let left = vertex(0x906, 2.0, 4.0);
        let upper = vertex(0x907, 4.0, 6.0);
        let right = vertex(0x908, 6.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let cuts = [
            edge(0x920, &left, &center, EdgeKind::Cut),
            edge(0x921, &center, &upper, EdgeKind::Cut),
            edge(0x922, &center, &right, EdgeKind::Cut),
        ];
        let mut edges = boundary_edges(&boundary, 0x910);
        edges.extend(cuts.iter().cloned());
        let pattern = CreasePattern {
            vertices: vec![
                a.clone(),
                b.clone(),
                c.clone(),
                d.clone(),
                center,
                left,
                upper,
                right,
            ],
            edges,
        };
        let source_paper = paper(&boundary, true);

        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("branched seam");
        assert_eq!(snapshot.faces.len(), 1);
        assert_eq!(snapshot.faces[0].holes.len(), 0);
        assert_eq!(snapshot.faces[0].seams.len(), 1);
        assert_eq!(snapshot.faces[0].seams[0].half_edges.len(), 6);
        assert_eq!(snapshot.material_components.len(), 1);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert!(cuts.iter().all(|cut| {
            matches!(
                snapshot
                    .edge_incidence
                    .iter()
                    .find(|(edge, _)| *edge == cut.id)
                    .map(|(_, incidence)| incidence),
                Some(EdgeIncidence::Cut { left, right }) if left == right
            )
        }));
    }

    #[test]
    fn boundary_connected_open_cut_is_removed_from_the_outer_cycle_and_kept_as_a_seam() {
        let namespace = fixed_id(1);
        let a = vertex(0xa01, 0.0, 0.0);
        let b = vertex(0xa02, 8.0, 0.0);
        let c = vertex(0xa03, 8.0, 8.0);
        let d = vertex(0xa04, 0.0, 8.0);
        let tip = vertex(0xa05, 4.0, 4.0);
        let boundary = [&a, &b, &c, &d];
        let cut = edge(0xa20, &a, &tip, EdgeKind::Cut);
        let mut edges = boundary_edges(&boundary, 0xa10);
        edges.push(cut.clone());
        let pattern = CreasePattern {
            vertices: vec![a.clone(), b.clone(), c.clone(), d.clone(), tip],
            edges,
        };
        let source_paper = paper(&boundary, true);

        let snapshot = extract_fold_graph_snapshot(input(namespace, &source_paper, &pattern))
            .expect("boundary-connected seam");
        assert_eq!(snapshot.faces.len(), 1);
        assert_eq!(snapshot.faces[0].outer.half_edges.len(), 4);
        assert_eq!(snapshot.faces[0].seams.len(), 1);
        assert_eq!(snapshot.faces[0].seams[0].half_edges.len(), 2);
        assert_eq!(snapshot.material_components.len(), 1);
        assert!(matches!(
            snapshot
                .edge_incidence
                .iter()
                .find(|(edge, _)| *edge == cut.id)
                .map(|(_, incidence)| incidence),
            Some(EdgeIncidence::Cut { left, right }) if left == right
        ));
    }

    fn hinge_faces(snapshot: &TopologySnapshot, edge: EdgeId) -> [FaceId; 2] {
        snapshot
            .edge_incidence
            .iter()
            .find_map(|(candidate, incidence)| {
                if *candidate != edge {
                    return None;
                }
                match incidence {
                    EdgeIncidence::Hinge { left, right, .. } => Some([*left, *right]),
                    _ => None,
                }
            })
            .expect("hinge incidence")
    }
}
