use ori_domain::{CreasePattern, Paper};

/// Produces the stable SHA-256 identity of the geometry that determines a fold
/// model.
///
/// The input is normalized as follows:
///
/// - vertex and edge records are ordered by canonical UUID bytes;
/// - edge endpoints are ordered because crease edges are undirected;
/// - the paper boundary is normalized across cyclic rotation and reversal;
/// - `f64` values use their exact IEEE-754 bits (including signed zero).
///
/// Project identity, revision, instruction data, names, colours, and texture
/// assets are intentionally outside this fingerprint's domain.
#[must_use]
pub fn fold_model_fingerprint_v1(pattern: &CreasePattern, paper: &Paper) -> String {
    ori_foldability::fold_model_fingerprint_v1(pattern, paper).to_hex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{
        Edge, EdgeId, EdgeKind, LengthDisplayUnit, Point2, ProjectId, Vertex, VertexId,
    };

    fn fixture() -> (CreasePattern, Paper) {
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        let edge = EdgeId::new();
        (
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: first,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: second,
                        position: Point2::new(1.0, 0.0),
                    },
                    Vertex {
                        id: third,
                        position: Point2::new(0.0, 1.0),
                    },
                ],
                edges: vec![Edge {
                    id: edge,
                    start: first,
                    end: second,
                    kind: EdgeKind::Mountain,
                }],
            },
            Paper {
                boundary_vertices: vec![first, second, third],
                thickness_mm: 0.1,
                cutting_allowed: false,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn fingerprint_is_lowercase_sha256_and_deterministic() {
        let (pattern, paper) = fixture();
        let first = fold_model_fingerprint_v1(&pattern, &paper);
        let second = fold_model_fingerprint_v1(&pattern, &paper);

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(first, first.to_ascii_lowercase());
    }

    #[test]
    fn storage_order_edge_direction_and_boundary_cycle_are_normalized() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);

        let mut reordered_pattern = pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        for edge in &mut reordered_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut rotated_paper = paper.clone();
        rotated_paper.boundary_vertices.rotate_left(1);
        rotated_paper.boundary_vertices.reverse();

        assert_eq!(
            fold_model_fingerprint_v1(&reordered_pattern, &rotated_paper),
            expected
        );
    }

    #[test]
    fn duplicate_invalid_records_do_not_reintroduce_storage_order_dependence() {
        let (mut pattern, paper) = fixture();
        pattern.vertices.push(Vertex {
            id: pattern.vertices[0].id,
            position: Point2::new(9.0, 8.0),
        });
        pattern.edges.push(Edge {
            id: pattern.edges[0].id,
            start: pattern.vertices[2].id,
            end: pattern.vertices[1].id,
            kind: EdgeKind::Auxiliary,
        });
        let expected = fold_model_fingerprint_v1(&pattern, &paper);
        pattern.vertices.reverse();
        pattern.edges.reverse();

        assert_eq!(fold_model_fingerprint_v1(&pattern, &paper), expected);
    }

    #[test]
    fn every_fold_model_field_changes_the_fingerprint() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);

        let mut changed = pattern.clone();
        changed.vertices[0].position.x = -0.0;
        assert_ne!(fold_model_fingerprint_v1(&changed, &paper), expected);

        let mut changed = pattern.clone();
        changed.edges[0].kind = EdgeKind::Valley;
        assert_ne!(fold_model_fingerprint_v1(&changed, &paper), expected);

        let mut changed = paper.clone();
        changed.cutting_allowed = true;
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);

        let mut changed = paper.clone();
        changed.thickness_mm = 0.2;
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);

        let mut changed = paper.clone();
        changed.boundary_vertices[0] = VertexId::new();
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);
    }

    #[test]
    fn presentation_settings_are_outside_the_fingerprint_domain() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);
        let mut changed = paper.clone();
        changed.front.color.red = 1;
        changed.back.color.alpha = 2;
        changed.length_display_unit = LengthDisplayUnit::PaperEdgeRatio {
            reference_edge: pattern.edges[0].id,
        };

        assert_eq!(fold_model_fingerprint_v1(&pattern, &changed), expected);
    }

    #[test]
    fn core_hex_fingerprint_matches_solver_provenance_exactly() {
        let sheet = crate::create_rectangular_sheet(8.0, 6.0, false).expect("rectangle");
        let (pattern, paper) = sheet.into_parts();
        let editor = crate::EditorState::with_paper(pattern, paper);
        let identity_namespace = ProjectId::new();
        let analyzed = editor.topology_analysis_input(identity_namespace).analyze();
        let topology = analyzed.simulation_snapshot().expect("rectangle topology");
        let local = ori_topology::analyze_local_flat_foldability(editor.paper(), editor.pattern());
        let report = ori_foldability::analyze_global_flat_foldability(
            ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
                identity_namespace,
                editor.paper(),
                editor.pattern(),
                topology,
                &local,
            ),
            ori_foldability::GlobalFlatFoldabilityLimits::default(),
        )
        .expect("solver executes");

        assert_eq!(
            report
                .provenance
                .source_fingerprint
                .expect("geometry-backed report fingerprint")
                .to_hex(),
            fold_model_fingerprint_v1(editor.pattern(), editor.paper())
        );
        assert_eq!(
            report.provenance.identity_namespace,
            Some(identity_namespace)
        );
    }
}
