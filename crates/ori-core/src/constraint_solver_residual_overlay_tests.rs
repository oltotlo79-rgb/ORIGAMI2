use ori_domain::{
    ConstraintId, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintRecordV1, Vertex,
};

use super::*;

struct OverlayFixture {
    pattern: CreasePattern,
    first_edge: EdgeId,
    second_edge: EdgeId,
    point: VertexId,
}

fn fixture() -> OverlayFixture {
    let vertices = std::array::from_fn::<_, 5, _>(|_| VertexId::new());
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    OverlayFixture {
        pattern: CreasePattern {
            vertices: vertices
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 1.0),
                    Point2::new(4.0, 0.0),
                    Point2::new(4.0, 3.0),
                    Point2::new(1.0, 2.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: vec![
                Edge {
                    id: first_edge,
                    start: vertices[0],
                    end: vertices[1],
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second_edge,
                    start: vertices[2],
                    end: vertices[3],
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        first_edge,
        second_edge,
        point: vertices[4],
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints
            .into_iter()
            .map(|constraint| GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            })
            .collect(),
    }
}

fn source_overlay(pattern: &CreasePattern) -> Vec<(VertexId, Point2)> {
    pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect()
}

fn collapse_edges(
    fixture: &OverlayFixture,
    edges: &[EdgeId],
    anchor: Point2,
) -> Vec<(VertexId, Point2)> {
    let collapsed = fixture
        .pattern
        .edges
        .iter()
        .filter(|edge| edges.contains(&edge.id))
        .flat_map(|edge| [edge.start, edge.end])
        .collect::<HashSet<_>>();
    fixture
        .pattern
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.id,
                if collapsed.contains(&vertex.id) {
                    anchor
                } else {
                    vertex.position
                },
            )
        })
        .collect()
}

#[test]
fn complete_overlay_certifies_signed_zero_and_is_storage_order_invariant() {
    let fixture = fixture();
    let constraints = document([
        GeometricConstraintKindV1::Horizontal {
            edge: fixture.first_edge,
        },
        GeometricConstraintKindV1::Vertical {
            edge: fixture.first_edge,
        },
    ]);
    for anchor in [Point2::new(0.0, 0.0), Point2::new(-0.0, -0.0)] {
        let overlay = collapse_edges(&fixture, &[fixture.first_edge], anchor);
        assert!(matches!(
            certify_binary64_residual_only_constraint_overlay_v1(
                &fixture.pattern,
                &constraints,
                &overlay,
            ),
            Ok(Some(_))
        ));
        let mut reversed_overlay = overlay.clone();
        reversed_overlay.reverse();
        assert!(matches!(
            certify_binary64_residual_only_constraint_overlay_v1(
                &fixture.pattern,
                &constraints,
                &reversed_overlay,
            ),
            Ok(Some(_))
        ));
    }

    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    for edge in &mut reversed_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut reversed_document = constraints.clone();
    reversed_document.constraints.reverse();
    let overlay = collapse_edges(&fixture, &[fixture.first_edge], Point2::new(16.0, 32.0));
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(
            &reversed_pattern,
            &reversed_document,
            &overlay,
        ),
        Ok(Some(_))
    ));
}

#[test]
fn overlay_registry_rejects_missing_duplicate_unknown_and_nonfinite_entries() {
    let fixture = fixture();
    let constraints = document([GeometricConstraintKindV1::Horizontal {
        edge: fixture.first_edge,
    }]);
    let complete = source_overlay(&fixture.pattern);
    let mut malformed = Vec::new();

    malformed.push(complete[..complete.len() - 1].to_vec());
    let mut duplicate = complete.clone();
    duplicate[1].0 = duplicate[0].0;
    malformed.push(duplicate);
    let mut unknown = complete.clone();
    unknown[0].0 = VertexId::new();
    malformed.push(unknown);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut nonfinite = complete.clone();
        nonfinite[0].1 = Point2::new(value, 0.0);
        malformed.push(nonfinite);
    }

    for overlay in malformed {
        assert!(matches!(
            certify_binary64_residual_only_constraint_overlay_v1(
                &fixture.pattern,
                &constraints,
                &overlay,
            ),
            Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
        ));
    }
}

#[test]
fn invalid_source_document_and_direct_conflict_never_issue_an_overlay_witness() {
    let fixture = fixture();
    let horizontal = GeometricConstraintKindV1::Horizontal {
        edge: fixture.first_edge,
    };
    let overlay = source_overlay(&fixture.pattern);

    let mut degenerate = fixture.pattern.clone();
    degenerate.vertices[1].position = degenerate.vertices[0].position;
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(
            &degenerate,
            &document([horizontal.clone()]),
            &overlay,
        ),
        Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
    ));
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(
            &fixture.pattern,
            &document([GeometricConstraintKindV1::Horizontal {
                edge: EdgeId::new(),
            }]),
            &overlay,
        ),
        Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
    ));

    let direct = document([
        horizontal,
        GeometricConstraintKindV1::Vertical {
            edge: fixture.first_edge,
        },
        GeometricConstraintKindV1::FixedLength {
            edge: fixture.first_edge,
            length_mm: 1.0,
        },
    ]);
    let collapsed = collapse_edges(&fixture, &[fixture.first_edge], Point2::new(0.0, 0.0));
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(&fixture.pattern, &direct, &collapsed,),
        Err(ConstraintSolveErrorV1::NonConvergent)
    ));
}

#[test]
fn collapsed_normalized_constraints_remain_nonfinite_and_fail_closed() {
    let fixture = fixture();
    let both_collapsed = collapse_edges(
        &fixture,
        &[fixture.first_edge, fixture.second_edge],
        Point2::new(0.0, 0.0),
    );
    let parallel = document([GeometricConstraintKindV1::Parallel {
        first_edge: fixture.first_edge,
        second_edge: fixture.second_edge,
    }]);
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(
            &fixture.pattern,
            &parallel,
            &both_collapsed,
        ),
        Err(ConstraintSolveErrorV1::NonConvergent)
    ));

    let point_on_line = document([GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.point,
        line_edge: fixture.first_edge,
    }]);
    let mut point_collapsed =
        collapse_edges(&fixture, &[fixture.first_edge], Point2::new(0.0, 0.0));
    point_collapsed
        .iter_mut()
        .find(|(vertex, _)| *vertex == fixture.point)
        .expect("point overlay entry")
        .1 = Point2::new(0.0, 0.0);
    assert!(matches!(
        certify_binary64_residual_only_constraint_overlay_v1(
            &fixture.pattern,
            &point_on_line,
            &point_collapsed,
        ),
        Err(ConstraintSolveErrorV1::NonConvergent)
    ));
}
