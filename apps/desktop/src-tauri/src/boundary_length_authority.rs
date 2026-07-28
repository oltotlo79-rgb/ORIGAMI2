//! Revision-bound native authority for paper-edge display lengths.
//!
//! The WebView uses these values for display-unit previews and expression
//! conversion. It never recomputes a boundary length with host `Math.hypot`.

use std::collections::{HashMap, HashSet};

use super::*;

pub(super) const BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1: u32 = 1;
pub(super) const BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1: &str =
    "ori_boundary_edge_length_binary64_native_v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct BoundaryLengthAuthorityV1 {
    schema_version: u32,
    model_id: &'static str,
    transcendental_model_id: &'static str,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    status: BoundaryLengthAuthorityStatusV1,
    entries: Vec<BoundaryLengthAuthorityEntryV1>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum BoundaryLengthAuthorityStatusV1 {
    Available,
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct BoundaryLengthAuthorityEntryV1 {
    boundary_index: usize,
    edge_id: EdgeId,
    start_vertex_id: VertexId,
    end_vertex_id: VertexId,
    length_mm: f64,
    length_bits_be: [u8; 8],
}

pub(super) fn derive_boundary_length_authority_v1(
    project: &ProjectState,
) -> BoundaryLengthAuthorityV1 {
    let unavailable = || BoundaryLengthAuthorityV1 {
        schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
        model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
        transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status: BoundaryLengthAuthorityStatusV1::Unavailable,
        entries: Vec::new(),
    };

    let pattern = project.editor.pattern();
    let boundary = &project.editor.paper().boundary_vertices;
    if boundary.len() < 3 {
        return unavailable();
    }

    let mut positions = HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        if !vertex.position.x.is_finite()
            || !vertex.position.y.is_finite()
            || positions.insert(vertex.id, vertex.position).is_some()
        {
            return unavailable();
        }
    }

    let mut boundary_vertices = HashSet::with_capacity(boundary.len());
    if boundary
        .iter()
        .any(|vertex| !boundary_vertices.insert(*vertex) || !positions.contains_key(vertex))
    {
        return unavailable();
    }

    let mut edge_ids = HashSet::with_capacity(pattern.edges.len());
    let mut boundary_edges = HashMap::<(VertexId, VertexId), Vec<&ori_domain::Edge>>::new();
    for edge in &pattern.edges {
        if !edge_ids.insert(edge.id)
            || edge.start == edge.end
            || !positions.contains_key(&edge.start)
            || !positions.contains_key(&edge.end)
        {
            return unavailable();
        }
        if edge.kind != EdgeKind::Boundary {
            continue;
        }
        boundary_edges
            .entry(canonical_vertex_pair_v1(edge.start, edge.end))
            .or_default()
            .push(edge);
    }
    if boundary_edges.len() != boundary.len()
        || boundary_edges.values().any(|edges| edges.len() != 1)
    {
        return unavailable();
    }

    let mut entries = Vec::with_capacity(boundary.len());
    for boundary_index in 0..boundary.len() {
        let start_vertex_id = boundary[boundary_index];
        let end_vertex_id = boundary[(boundary_index + 1) % boundary.len()];
        let key = canonical_vertex_pair_v1(start_vertex_id, end_vertex_id);
        let Some([edge]) = boundary_edges.get(&key).map(Vec::as_slice) else {
            return unavailable();
        };
        let start = positions[&start_vertex_id];
        let end = positions[&end_vertex_id];
        let Ok(length_mm) = ori_numeric::deterministic_hypot_v1(end.x - start.x, end.y - start.y)
        else {
            return unavailable();
        };
        if length_mm <= 0.0 {
            return unavailable();
        }
        entries.push(BoundaryLengthAuthorityEntryV1 {
            boundary_index,
            edge_id: edge.id,
            start_vertex_id,
            end_vertex_id,
            length_mm,
            length_bits_be: length_mm.to_bits().to_be_bytes(),
        });
    }

    BoundaryLengthAuthorityV1 {
        schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
        model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
        transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status: BoundaryLengthAuthorityStatusV1::Available,
        entries,
    }
}

fn canonical_vertex_pair_v1(first: VertexId, second: VertexId) -> (VertexId, VertexId) {
    if first.canonical_bytes() <= second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{CreasePattern, Edge, Paper, Vertex};

    fn vertex(id: VertexId, x: f64, y: f64) -> Vertex {
        Vertex {
            id,
            position: Point2::new(x, y),
        }
    }

    fn authority_for(pattern: CreasePattern, paper: Paper) -> BoundaryLengthAuthorityV1 {
        derive_boundary_length_authority_v1(&ProjectState::new_with_paper(pattern, paper))
    }

    #[test]
    fn valid_boundary_lengths_are_native_bit_bound_and_in_boundary_order() {
        let sheet = create_rectangular_sheet(3.0, 4.0, false).expect("sheet");
        let (pattern, paper) = sheet.into_parts();
        let authority = authority_for(pattern, paper);
        assert!(matches!(
            authority.status,
            BoundaryLengthAuthorityStatusV1::Available
        ));
        assert_eq!(authority.entries.len(), 4);
        for (index, entry) in authority.entries.iter().enumerate() {
            assert_eq!(entry.boundary_index, index);
            assert_eq!(
                entry.length_bits_be,
                entry.length_mm.to_bits().to_be_bytes()
            );
            assert!(entry.length_mm == 3.0 || entry.length_mm == 4.0);
        }
    }

    #[test]
    fn one_ulp_geometry_uses_the_frozen_hypot_result() {
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let third_edge = EdgeId::new();
        let one_above = f64::from_bits(1.0_f64.to_bits() + 1);
        let pattern = CreasePattern {
            vertices: vec![
                vertex(first, 0.0, 0.0),
                vertex(second, one_above, 1.0),
                vertex(third, 0.0, 2.0),
            ],
            edges: vec![
                Edge {
                    id: first_edge,
                    start: first,
                    end: second,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: second_edge,
                    start: second,
                    end: third,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: third_edge,
                    start: third,
                    end: first,
                    kind: EdgeKind::Boundary,
                },
            ],
        };
        let paper = Paper {
            boundary_vertices: vec![first, second, third],
            ..Paper::default()
        };
        let authority = authority_for(pattern, paper);
        let expected = ori_numeric::deterministic_hypot_v1(one_above, 1.0).expect("finite hypot");
        assert!(matches!(
            authority.status,
            BoundaryLengthAuthorityStatusV1::Available
        ));
        assert_eq!(authority.entries[0].edge_id, first_edge);
        assert_eq!(
            authority.entries[0].length_bits_be,
            expected.to_bits().to_be_bytes()
        );
    }

    #[test]
    fn execute_undo_redo_and_reopen_refresh_the_revision_bound_authority() {
        let sheet = create_rectangular_sheet(3.0, 4.0, false).expect("sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let start_vertex_id = project.editor.paper().boundary_vertices[0];
        let end_vertex_id = project.editor.paper().boundary_vertices[1];
        let start = project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == start_vertex_id)
            .expect("start vertex")
            .position;
        let end = project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == end_vertex_id)
            .expect("end vertex")
            .position;
        let initial = derive_boundary_length_authority_v1(&project);
        let initial_length = initial.entries[0].length_mm;
        assert_eq!(initial.revision, 0);

        let moved = project
            .editor
            .execute(
                0,
                Command::MoveVertex {
                    id: end_vertex_id,
                    position: Point2::new(
                        start.x + 2.0 * (end.x - start.x),
                        start.y + 2.0 * (end.y - start.y),
                    ),
                },
            )
            .expect("move boundary vertex");
        let moved_authority = derive_boundary_length_authority_v1(&project);
        assert_eq!(moved_authority.revision, moved.revision);
        assert_eq!(moved_authority.entries[0].length_mm, initial_length * 2.0);

        let undone = project.editor.undo(moved.revision).expect("undo move");
        let undone_authority = derive_boundary_length_authority_v1(&project);
        assert_eq!(undone_authority.revision, undone.revision);
        assert_eq!(undone_authority.entries[0].length_mm, initial_length);

        let redone = project.editor.redo(undone.revision).expect("redo move");
        let redone_authority = derive_boundary_length_authority_v1(&project);
        assert_eq!(redone_authority.revision, redone.revision);
        assert_eq!(redone_authority.entries[0].length_mm, initial_length * 2.0);

        let reopened = ProjectState::from_valid_document(
            project.document(),
            std::path::PathBuf::from("boundary-authority.ori2"),
        );
        let reopened_authority = derive_boundary_length_authority_v1(&reopened);
        assert_eq!(reopened_authority.revision, 0);
        assert_eq!(
            reopened_authority.entries[0].length_mm,
            redone_authority.entries[0].length_mm
        );
        assert_eq!(
            reopened_authority.entries[0].length_bits_be,
            redone_authority.entries[0].length_bits_be
        );
    }

    #[test]
    fn duplicate_or_missing_boundary_authority_fails_closed() {
        let sheet = create_rectangular_sheet(3.0, 4.0, false).expect("sheet");
        let (pattern, paper) = sheet.into_parts();

        let mut duplicate_id = pattern.clone();
        duplicate_id.edges.push(duplicate_id.edges[0].clone());
        let duplicate = authority_for(duplicate_id, paper.clone());
        assert!(matches!(
            duplicate.status,
            BoundaryLengthAuthorityStatusV1::Unavailable
        ));
        assert!(duplicate.entries.is_empty());

        let mut missing = pattern;
        missing.edges.remove(0);
        let missing = authority_for(missing, paper);
        assert!(matches!(
            missing.status,
            BoundaryLengthAuthorityStatusV1::Unavailable
        ));
        assert!(missing.entries.is_empty());

        let sheet = create_rectangular_sheet(3.0, 4.0, false).expect("sheet");
        let (mut extra, paper) = sheet.into_parts();
        extra.edges.push(Edge {
            id: EdgeId::new(),
            start: extra.vertices[0].id,
            end: extra.vertices[2].id,
            kind: EdgeKind::Boundary,
        });
        let extra = authority_for(extra, paper);
        assert!(matches!(
            extra.status,
            BoundaryLengthAuthorityStatusV1::Unavailable
        ));
        assert!(extra.entries.is_empty());

        let sheet = create_rectangular_sheet(3.0, 4.0, false).expect("sheet");
        let (mut dangling, paper) = sheet.into_parts();
        dangling.edges.push(Edge {
            id: EdgeId::new(),
            start: dangling.vertices[0].id,
            end: VertexId::new(),
            kind: EdgeKind::Mountain,
        });
        let dangling = authority_for(dangling, paper);
        assert!(matches!(
            dangling.status,
            BoundaryLengthAuthorityStatusV1::Unavailable
        ));
        assert!(dangling.entries.is_empty());
    }
}
