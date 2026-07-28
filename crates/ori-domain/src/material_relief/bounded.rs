use std::collections::{HashMap, HashSet};

use crate::{CreasePattern, EdgeId, EdgeKind, Paper, ProjectId, VertexId};

use super::{
    FramedSha256, MATERIAL_RELIEF_DOCUMENT_VERSION_V1, MATERIAL_RELIEF_GEOMETRY_HASH_DOMAIN_V1,
    MATERIAL_RELIEF_STATE_HASH_DOMAIN_V1, MATERIAL_RELIEF_SUBSTRATE_HASH_DOMAIN_V1,
    MAX_MATERIAL_RELIEF_LOOP_EDGES_V1, MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1,
    MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1, MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1,
    MAX_MATERIAL_RELIEF_REGIONS_V1, MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1,
    MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1, MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1,
    MaterialReliefDocumentV1, MaterialReliefDocumentValidationErrorV1, MaterialReliefLineageId,
    MaterialReliefRegionV1,
};

pub(super) fn preflight_material_relief_substrate_collections_v1(
    vertex_count: usize,
    edge_count: usize,
    paper_boundary_vertex_count: usize,
) -> Result<(), MaterialReliefDocumentValidationErrorV1> {
    use MaterialReliefDocumentValidationErrorV1 as ValidationError;

    if vertex_count > MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1 {
        return Err(ValidationError::TooManyPatternVertices {
            actual: vertex_count,
            maximum: MAX_MATERIAL_RELIEF_PATTERN_VERTICES_V1,
        });
    }
    if edge_count > MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1 {
        return Err(ValidationError::TooManyPatternEdges {
            actual: edge_count,
            maximum: MAX_MATERIAL_RELIEF_PATTERN_EDGES_V1,
        });
    }
    if paper_boundary_vertex_count > MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1 {
        return Err(ValidationError::TooManyPaperBoundaryVertices {
            actual: paper_boundary_vertex_count,
            maximum: MAX_MATERIAL_RELIEF_PAPER_BOUNDARY_VERTICES_V1,
        });
    }
    Ok(())
}

pub(super) fn preflight_material_relief_region_collections_v1(
    regions: &[MaterialReliefRegionV1],
) -> Result<(usize, usize), MaterialReliefDocumentValidationErrorV1> {
    use MaterialReliefDocumentValidationErrorV1 as ValidationError;

    if regions.len() > MAX_MATERIAL_RELIEF_REGIONS_V1 {
        return Err(ValidationError::TooManyRegions {
            actual: regions.len(),
            maximum: MAX_MATERIAL_RELIEF_REGIONS_V1,
        });
    }
    let total_removed_components = checked_total(
        regions
            .iter()
            .map(|region| region.removed_component_keys.len()),
    )?;
    if total_removed_components > MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1 {
        return Err(ValidationError::TooManyTotalRemovedComponents {
            actual: total_removed_components,
            maximum: MAX_MATERIAL_RELIEF_TOTAL_REMOVED_COMPONENTS_V1,
        });
    }
    for (region_index, region) in regions.iter().enumerate() {
        if region.removed_component_keys.len() > MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1 {
            return Err(ValidationError::TooManyRemovedComponents {
                region_index,
                actual: region.removed_component_keys.len(),
                maximum: MAX_MATERIAL_RELIEF_REMOVED_COMPONENTS_V1,
            });
        }
        if region.boundary_edge_loop.len() > MAX_MATERIAL_RELIEF_LOOP_EDGES_V1 {
            return Err(ValidationError::TooManyLoopEdges {
                region_index,
                actual: region.boundary_edge_loop.len(),
                maximum: MAX_MATERIAL_RELIEF_LOOP_EDGES_V1,
            });
        }
    }
    let total_loop_edges =
        checked_total(regions.iter().map(|region| region.boundary_edge_loop.len()))?;
    if total_loop_edges > MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1 {
        return Err(ValidationError::TooManyTotalLoopEdges {
            actual: total_loop_edges,
            maximum: MAX_MATERIAL_RELIEF_TOTAL_LOOP_EDGES_V1,
        });
    }
    Ok((total_removed_components, total_loop_edges))
}

/// Computes the revision-independent substrate identity used to bind relief.
///
/// Vertex/edge storage order, undirected edge direction, and the paper
/// boundary's cycle start/direction are normalized. The hash includes exact
/// binary64 coordinate bits and cutting policy. It does not directly frame an
/// explicit project or revision ID, but it deliberately includes stable
/// vertex/edge IDs (including paper-boundary vertex IDs), which may themselves
/// have been derived from a project namespace. Consequently this is stable
/// across ID-preserving copies, not arbitrary geometric re-keying. Material
/// relief, thickness, display units, and appearance are excluded.
///
/// Collection ceilings are checked before collection-sized scratch storage is
/// allocated. Allocation failure is returned rather than hidden behind an
/// infallible `collect`.
pub fn material_relief_substrate_sha256_v1(
    pattern: &CreasePattern,
    paper: &Paper,
) -> Result<[u8; 32], MaterialReliefDocumentValidationErrorV1> {
    preflight_material_relief_substrate_collections_v1(
        pattern.vertices.len(),
        pattern.edges.len(),
        paper.boundary_vertices.len(),
    )?;

    let mut hash = FramedSha256::new(MATERIAL_RELIEF_SUBSTRATE_HASH_DOMAIN_V1);
    hash.frame(&[MATERIAL_RELIEF_DOCUMENT_VERSION_V1]);

    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(pattern.vertices.len())
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    for vertex in &pattern.vertices {
        if vertex.id.canonical_bytes() == [0; 16] {
            return Err(MaterialReliefDocumentValidationErrorV1::NilPatternVertex {
                vertex: vertex.id,
            });
        }
        vertices.push(vertex);
    }
    vertices.sort_unstable_by_key(|vertex| {
        (
            vertex.id.canonical_bytes(),
            vertex.position.x.to_bits(),
            vertex.position.y.to_bits(),
        )
    });
    hash.frame_usize(vertices.len());
    for vertex in vertices {
        hash.frame(&vertex.id.canonical_bytes());
        hash.frame(&vertex.position.x.to_bits().to_be_bytes());
        hash.frame(&vertex.position.y.to_bits().to_be_bytes());
    }

    let mut edges = Vec::new();
    edges
        .try_reserve_exact(pattern.edges.len())
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    for edge in &pattern.edges {
        if edge.id.canonical_bytes() == [0; 16] {
            return Err(MaterialReliefDocumentValidationErrorV1::NilPatternEdge { edge: edge.id });
        }
        edges.push(edge);
    }
    edges.sort_unstable_by_key(|edge| {
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        (
            edge.id.canonical_bytes(),
            endpoints,
            edge_kind_tag(edge.kind),
        )
    });
    hash.frame_usize(edges.len());
    for edge in edges {
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        hash.frame(&edge.id.canonical_bytes());
        hash.frame(&endpoints[0]);
        hash.frame(&endpoints[1]);
        hash.frame(&[edge_kind_tag(edge.kind)]);
    }

    let boundary = canonical_boundary(&paper.boundary_vertices)?;
    hash.frame_usize(boundary.len());
    for vertex in boundary {
        hash.frame(&vertex.canonical_bytes());
    }
    hash.frame(&[u8::from(paper.cutting_allowed)]);
    Ok(hash.finish())
}

/// Computes the explicit-project-field-independent relief-geometry digest.
///
/// Lineage and source project IDs are intentionally excluded, while the
/// substrate, requested roots, reconstructed closures, loops, order, and
/// region boundaries are bound. Because the substrate digest includes stable
/// element IDs, this digest is project-independent only for ID-preserving
/// copies; it is not invariant under vertex/edge re-keying. The caller must
/// validate canonical order separately. All region collection ceilings are
/// checked before hashing.
pub fn material_relief_geometry_sha256_v1(
    substrate_fingerprint_sha256: [u8; 32],
    regions: &[MaterialReliefRegionV1],
) -> Result<[u8; 32], MaterialReliefDocumentValidationErrorV1> {
    preflight_material_relief_region_collections_v1(regions)?;
    let mut hash = FramedSha256::new(MATERIAL_RELIEF_GEOMETRY_HASH_DOMAIN_V1);
    hash.frame(&[MATERIAL_RELIEF_DOCUMENT_VERSION_V1]);
    hash.frame(&substrate_fingerprint_sha256);
    hash.frame_usize(regions.len());
    for region in regions {
        hash_region_geometry(&mut hash, region);
    }
    Ok(hash.finish())
}

/// Computes the complete persisted-state digest.
///
/// Unlike [`material_relief_geometry_sha256_v1`], this digest binds the source
/// project and every deterministic lineage. Runtime revision and caller limits
/// are deliberately excluded so canonical persisted inputs remain stable across
/// reopen. Arbitrary region or loop reordering is rejected rather than
/// normalized.
pub fn material_relief_state_sha256_v1(
    source_project_id: ProjectId,
    substrate_fingerprint_sha256: [u8; 32],
    regions: &[MaterialReliefRegionV1],
) -> Result<[u8; 32], MaterialReliefDocumentValidationErrorV1> {
    preflight_material_relief_region_collections_v1(regions)?;
    let mut hash = FramedSha256::new(MATERIAL_RELIEF_STATE_HASH_DOMAIN_V1);
    hash.frame(&[MATERIAL_RELIEF_DOCUMENT_VERSION_V1]);
    hash.frame(&source_project_id.canonical_bytes());
    hash.frame(&substrate_fingerprint_sha256);
    hash.frame_usize(regions.len());
    for region in regions {
        hash.frame(&region.lineage_id.canonical_bytes());
        hash_region_geometry(&mut hash, region);
    }
    Ok(hash.finish())
}

/// Strictly validates a persisted relief envelope against its current
/// substrate.
///
/// Success is evidence of canonical, internally bound wire data only; it does
/// not grant any authority exposed by [`MaterialReliefDocumentV1`]. This
/// function has no live-project ID parameter, so success also does not prove
/// that `source_project_id` equals a current container or editor identity; a
/// future consumer must bind that separately before reconstructing topology.
pub fn validate_material_relief_document_v1(
    document: &MaterialReliefDocumentV1,
    pattern: &CreasePattern,
    paper: &Paper,
) -> Result<(), MaterialReliefDocumentValidationErrorV1> {
    use MaterialReliefDocumentValidationErrorV1 as ValidationError;

    if document.version != MATERIAL_RELIEF_DOCUMENT_VERSION_V1 {
        return Err(ValidationError::UnsupportedVersion {
            actual: document.version,
            expected: MATERIAL_RELIEF_DOCUMENT_VERSION_V1,
        });
    }
    if document.regions.is_empty() {
        return if document.is_default() {
            Ok(())
        } else {
            Err(ValidationError::NonDefaultEmptyDocument)
        };
    }
    let (total_removed_components, total_loop_edges) =
        preflight_material_relief_region_collections_v1(&document.regions)?;
    preflight_material_relief_substrate_collections_v1(
        pattern.vertices.len(),
        pattern.edges.len(),
        paper.boundary_vertices.len(),
    )?;
    if !paper.cutting_allowed {
        return Err(ValidationError::CuttingNotAllowed);
    }
    let source_project_id = document
        .source_project_id
        .ok_or(ValidationError::MissingSourceProjectId)?;
    if source_project_id.canonical_bytes() == [0; 16] {
        return Err(ValidationError::NilSourceProjectId);
    }
    if document.substrate_fingerprint_sha256 == [0; 32] {
        return Err(ValidationError::ZeroSubstrateFingerprint);
    }
    if document.state_sha256 == [0; 32] {
        return Err(ValidationError::ZeroStateDigest);
    }

    for (region_index, region) in document.regions.iter().enumerate() {
        if region.removed_component_keys.is_empty() {
            return Err(ValidationError::EmptyRemovedComponents { region_index });
        }
        if region.boundary_edge_loop.len() < 3 {
            return Err(ValidationError::InvalidBoundaryLoop { region_index });
        }
    }

    let edge_by_id = validate_and_index_pattern(pattern, paper)?;

    let expected_substrate = material_relief_substrate_sha256_v1(pattern, paper)?;
    if document.substrate_fingerprint_sha256 != expected_substrate {
        return Err(ValidationError::SubstrateFingerprintMismatch);
    }

    let mut lineage_ids = HashSet::new();
    lineage_ids
        .try_reserve(document.regions.len())
        .map_err(|_| ValidationError::ResourceAllocation)?;
    let mut requested_component_keys = HashSet::new();
    requested_component_keys
        .try_reserve(document.regions.len())
        .map_err(|_| ValidationError::ResourceAllocation)?;
    for (region_index, region) in document.regions.iter().enumerate() {
        if !lineage_ids.insert(region.lineage_id) {
            return Err(ValidationError::DuplicateLineageId { region_index });
        }
        if !requested_component_keys.insert(region.requested_component_key) {
            return Err(ValidationError::DuplicateRequestedComponentKey { region_index });
        }
    }
    if let Some((region_index, _)) = document.regions.windows(2).enumerate().find(|(_, pair)| {
        pair[0].requested_component_key.as_slice() >= pair[1].requested_component_key.as_slice()
    }) {
        return Err(ValidationError::NonCanonicalRegionOrder {
            region_index: region_index + 1,
        });
    }

    let mut removed_component_keys = HashSet::new();
    removed_component_keys
        .try_reserve(total_removed_components)
        .map_err(|_| ValidationError::ResourceAllocation)?;
    let mut boundary_edge_ids = HashSet::new();
    boundary_edge_ids
        .try_reserve(total_loop_edges)
        .map_err(|_| ValidationError::ResourceAllocation)?;

    for (region_index, region) in document.regions.iter().enumerate() {
        if region.lineage_id.is_nil() {
            return Err(ValidationError::NilLineageId { region_index });
        }
        if region.requested_component_key == [0; 32] {
            return Err(ValidationError::ZeroRequestedComponentKey { region_index });
        }
        let expected_lineage = MaterialReliefLineageId::derive_v5(
            source_project_id,
            document.substrate_fingerprint_sha256,
            region.requested_component_key,
        );
        if region.lineage_id != expected_lineage {
            return Err(ValidationError::InvalidLineageId { region_index });
        }
        if region.removed_component_keys.contains(&[0; 32])
            || region
                .removed_component_keys
                .windows(2)
                .any(|pair| pair[0].as_slice() >= pair[1].as_slice())
        {
            return Err(ValidationError::RemovedComponentsNotCanonical { region_index });
        }
        if region
            .removed_component_keys
            .binary_search(&region.requested_component_key)
            .is_err()
        {
            return Err(ValidationError::RequestedComponentMissingFromClosure { region_index });
        }
        if region
            .removed_component_keys
            .iter()
            .any(|key| !removed_component_keys.insert(*key))
        {
            return Err(ValidationError::RemovedComponentClosureOverlap { region_index });
        }
        if !is_canonical_loop(&region.boundary_edge_loop) {
            return Err(ValidationError::InvalidBoundaryLoop { region_index });
        }

        let mut endpoints = Vec::new();
        endpoints
            .try_reserve_exact(region.boundary_edge_loop.len())
            .map_err(|_| ValidationError::ResourceAllocation)?;
        for edge_id in &region.boundary_edge_loop {
            if !boundary_edge_ids.insert(*edge_id) {
                return Err(ValidationError::BoundaryEdgeReused {
                    region_index,
                    edge: *edge_id,
                });
            }
            let edge = edge_by_id
                .get(edge_id)
                .ok_or(ValidationError::UnknownBoundaryEdge {
                    region_index,
                    edge: *edge_id,
                })?;
            if edge.kind != EdgeKind::Cut {
                return Err(ValidationError::NonCutBoundaryEdge {
                    region_index,
                    edge: *edge_id,
                });
            }
            endpoints.push((edge.start, edge.end));
        }
        validate_connected_cycle(&endpoints, region_index)?;
    }

    let expected_state = material_relief_state_sha256_v1(
        source_project_id,
        document.substrate_fingerprint_sha256,
        &document.regions,
    )?;
    if document.state_sha256 != expected_state {
        return Err(ValidationError::StateDigestMismatch);
    }
    Ok(())
}

fn validate_and_index_pattern<'a>(
    pattern: &'a CreasePattern,
    paper: &Paper,
) -> Result<HashMap<EdgeId, &'a crate::Edge>, MaterialReliefDocumentValidationErrorV1> {
    use MaterialReliefDocumentValidationErrorV1 as ValidationError;

    let mut vertices = HashSet::new();
    vertices
        .try_reserve(pattern.vertices.len())
        .map_err(|_| ValidationError::ResourceAllocation)?;
    for vertex in &pattern.vertices {
        if vertex.id.canonical_bytes() == [0; 16] {
            return Err(ValidationError::NilPatternVertex { vertex: vertex.id });
        }
        if !vertices.insert(vertex.id) {
            return Err(ValidationError::DuplicatePatternVertex { vertex: vertex.id });
        }
        if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
            return Err(ValidationError::NonFinitePatternVertex { vertex: vertex.id });
        }
    }
    if paper.boundary_vertices.len() < 3 {
        return Err(ValidationError::InvalidPaperBoundary);
    }
    for vertex in &paper.boundary_vertices {
        if !vertices.contains(vertex) {
            return Err(ValidationError::PaperBoundaryReferencesUnknownVertex { vertex: *vertex });
        }
    }
    let mut boundary_vertices = HashSet::new();
    boundary_vertices
        .try_reserve(paper.boundary_vertices.len())
        .map_err(|_| ValidationError::ResourceAllocation)?;
    for vertex in &paper.boundary_vertices {
        if !boundary_vertices.insert(*vertex) {
            return Err(ValidationError::DuplicatePaperBoundaryVertex { vertex: *vertex });
        }
    }

    let mut edge_by_id = HashMap::new();
    edge_by_id
        .try_reserve(pattern.edges.len())
        .map_err(|_| ValidationError::ResourceAllocation)?;
    for edge in &pattern.edges {
        if edge.id.canonical_bytes() == [0; 16] {
            return Err(ValidationError::NilPatternEdge { edge: edge.id });
        }
        if !vertices.contains(&edge.start) || !vertices.contains(&edge.end) {
            return Err(ValidationError::PatternEdgeReferencesUnknownVertex { edge: edge.id });
        }
        if edge.start == edge.end {
            return Err(ValidationError::DegeneratePatternEdge { edge: edge.id });
        }
        if edge_by_id.insert(edge.id, edge).is_some() {
            return Err(ValidationError::DuplicatePatternEdge { edge: edge.id });
        }
    }
    Ok(edge_by_id)
}

pub(super) fn checked_total(
    lengths: impl IntoIterator<Item = usize>,
) -> Result<usize, MaterialReliefDocumentValidationErrorV1> {
    lengths
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .ok_or(MaterialReliefDocumentValidationErrorV1::ResourceAllocation)
}

fn hash_region_geometry(hash: &mut FramedSha256, region: &MaterialReliefRegionV1) {
    hash.frame(&region.requested_component_key);
    hash.frame_usize(region.removed_component_keys.len());
    for component_key in &region.removed_component_keys {
        hash.frame(component_key);
    }
    hash.frame_usize(region.boundary_edge_loop.len());
    for edge in &region.boundary_edge_loop {
        hash.frame(&edge.canonical_bytes());
    }
}

const fn edge_kind_tag(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Mountain => 0,
        EdgeKind::Valley => 1,
        EdgeKind::Auxiliary => 2,
        EdgeKind::Boundary => 3,
        EdgeKind::Cut => 4,
    }
}

pub(super) fn is_canonical_loop(edges: &[EdgeId]) -> bool {
    if edges.len() < 3 {
        return false;
    }
    let first = edges[0].canonical_bytes();
    edges
        .iter()
        .skip(1)
        .all(|edge| first < edge.canonical_bytes())
        && edges[1].canonical_bytes() < edges[edges.len() - 1].canonical_bytes()
}

fn validate_connected_cycle(
    endpoints: &[(VertexId, VertexId)],
    region_index: usize,
) -> Result<(), MaterialReliefDocumentValidationErrorV1> {
    let degree_capacity = checked_cycle_degree_capacity(endpoints.len())?;
    let mut degree = HashMap::<VertexId, usize>::new();
    degree
        .try_reserve(degree_capacity)
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    for (start, end) in endpoints {
        *degree.entry(*start).or_default() += 1;
        *degree.entry(*end).or_default() += 1;
    }
    if degree.len() != endpoints.len() || degree.values().any(|degree| *degree != 2) {
        return Err(MaterialReliefDocumentValidationErrorV1::InvalidBoundaryLoop { region_index });
    }
    for index in 0..endpoints.len() {
        let first = endpoints[index];
        let second = endpoints[(index + 1) % endpoints.len()];
        let shared = [first.0, first.1]
            .into_iter()
            .filter(|vertex| *vertex == second.0 || *vertex == second.1)
            .count();
        if shared != 1 {
            return Err(
                MaterialReliefDocumentValidationErrorV1::InvalidBoundaryLoop { region_index },
            );
        }
    }
    Ok(())
}

pub(super) fn checked_cycle_degree_capacity(
    edge_count: usize,
) -> Result<usize, MaterialReliefDocumentValidationErrorV1> {
    edge_count
        .checked_mul(2)
        .ok_or(MaterialReliefDocumentValidationErrorV1::ResourceAllocation)
}

fn canonical_boundary(
    boundary: &[VertexId],
) -> Result<Vec<VertexId>, MaterialReliefDocumentValidationErrorV1> {
    let mut forward_bytes = Vec::new();
    forward_bytes
        .try_reserve_exact(boundary.len())
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    forward_bytes.extend(boundary.iter().map(VertexId::canonical_bytes));
    if boundary.len() < 2 {
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(boundary.len())
            .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
        canonical.extend(boundary.iter().copied());
        return Ok(canonical);
    }

    let mut reverse_bytes = Vec::new();
    reverse_bytes
        .try_reserve_exact(boundary.len())
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    reverse_bytes.extend(forward_bytes.iter().copied().rev());
    let forward_start = least_rotation_start(&forward_bytes);
    let reverse_start = least_rotation_start(&reverse_bytes);
    let use_forward = (0..boundary.len())
        .find_map(|offset| {
            let forward = forward_bytes[(forward_start + offset) % boundary.len()];
            let reverse = reverse_bytes[(reverse_start + offset) % boundary.len()];
            match forward.cmp(&reverse) {
                std::cmp::Ordering::Equal => None,
                ordering => Some(ordering.is_lt()),
            }
        })
        .unwrap_or(true);

    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(boundary.len())
        .map_err(|_| MaterialReliefDocumentValidationErrorV1::ResourceAllocation)?;
    if use_forward {
        for offset in 0..boundary.len() {
            canonical.push(boundary[(forward_start + offset) % boundary.len()]);
        }
    } else {
        for offset in 0..boundary.len() {
            let reverse_index = (reverse_start + offset) % boundary.len();
            canonical.push(boundary[boundary.len() - 1 - reverse_index]);
        }
    }
    Ok(canonical)
}

/// Booth's algorithm for the lexicographically least cyclic rotation.
fn least_rotation_start<T: Ord>(values: &[T]) -> usize {
    let len = values.len();
    if len < 2 {
        return 0;
    }
    let (mut first, mut second, mut offset) = (0, 1, 0);
    while first < len && second < len && offset < len {
        use std::cmp::Ordering;
        match values[(first + offset) % len].cmp(&values[(second + offset) % len]) {
            Ordering::Equal => offset += 1,
            Ordering::Greater => {
                first += offset + 1;
                if first == second {
                    first += 1;
                }
                offset = 0;
            }
            Ordering::Less => {
                second += offset + 1;
                if first == second {
                    second += 1;
                }
                offset = 0;
            }
        }
    }
    first.min(second) % len
}
