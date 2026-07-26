use std::collections::BTreeMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::construct_single_constraint_exact_assignment_v1;
use crate::certify_binary64_exact_geometric_constraint_satisfaction_v1;

struct FixtureBuilder {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
}

impl FixtureBuilder {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn vertex(&mut self) -> VertexId {
        let ordinal = self.vertices.len() as f64;
        let id = VertexId::new();
        self.vertices.push(Vertex {
            id,
            position: Point2::new(ordinal * 3.0 + 1.0, ordinal * 5.0 + 2.0),
        });
        id
    }

    fn edge(&mut self, start: VertexId, end: VertexId) -> EdgeId {
        let id = EdgeId::new();
        self.edges.push(Edge {
            id,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        });
        id
    }

    fn finish(self, constraint: GeometricConstraintKindV1) -> ConstructiveCase {
        ConstructiveCase {
            pattern: CreasePattern {
                vertices: self.vertices,
                edges: self.edges,
            },
            document: document([record(constraint)]),
        }
    }
}

struct ConstructiveCase {
    pattern: CreasePattern,
    document: GeometricConstraintDocumentV1,
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
}

fn one_edge(make: impl FnOnce(EdgeId) -> GeometricConstraintKindV1) -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let start = builder.vertex();
    let end = builder.vertex();
    let edge = builder.edge(start, end);
    builder.finish(make(edge))
}

fn two_edges(make: impl FnOnce(EdgeId, EdgeId) -> GeometricConstraintKindV1) -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let shared = builder.vertex();
    let first_other = builder.vertex();
    let second_other = builder.vertex();
    let first = builder.edge(shared, first_other);
    let second = builder.edge(second_other, shared);
    builder.finish(make(first, second))
}

fn fixed_angle_case(angle_degrees: f64) -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let vertex = builder.vertex();
    let first_other = builder.vertex();
    let second_other = builder.vertex();
    let first = builder.edge(vertex, first_other);
    let second = builder.edge(second_other, vertex);
    builder.finish(GeometricConstraintKindV1::FixedAngle {
        vertex,
        first_edge: first,
        second_edge: second,
        angle_degrees,
    })
}

fn point_on_line_case() -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let start = builder.vertex();
    let end = builder.vertex();
    let point = builder.vertex();
    let line = builder.edge(end, start);
    builder.finish(GeometricConstraintKindV1::PointOnLine {
        vertex: point,
        line_edge: line,
    })
}

fn mirror_case() -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let axis_start = builder.vertex();
    let axis_end = builder.vertex();
    let first = builder.vertex();
    let second = builder.vertex();
    let axis = builder.edge(axis_end, axis_start);
    builder.finish(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: first,
        second_vertex: second,
        axis_edge: axis,
    })
}

fn rotation_case(angle_degrees: f64) -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let center = builder.vertex();
    let source = builder.vertex();
    let target = builder.vertex();
    builder.finish(GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex: center,
        source_vertex: source,
        target_vertex: target,
        angle_degrees,
    })
}

fn angle_bisector_case() -> ConstructiveCase {
    let mut builder = FixtureBuilder::new();
    let vertex = builder.vertex();
    let first_other = builder.vertex();
    let second_other = builder.vertex();
    let bisector_other = builder.vertex();
    let first = builder.edge(vertex, first_other);
    let second = builder.edge(second_other, vertex);
    let bisector = builder.edge(vertex, bisector_other);
    builder.finish(GeometricConstraintKindV1::AngleBisector {
        vertex,
        first_edge: first,
        second_edge: second,
        bisector_edge: bisector,
    })
}

fn positive_cases() -> Vec<(&'static str, ConstructiveCase)> {
    vec![
        (
            "fixed_length",
            one_edge(|edge| GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 3.0,
            }),
        ),
        ("fixed_angle", fixed_angle_case(60.0)),
        (
            "horizontal",
            one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge }),
        ),
        (
            "vertical",
            one_edge(|edge| GeometricConstraintKindV1::Vertical { edge }),
        ),
        (
            "equal_length",
            two_edges(
                |first_edge, second_edge| GeometricConstraintKindV1::EqualLength {
                    first_edge,
                    second_edge,
                },
            ),
        ),
        (
            "parallel",
            two_edges(
                |first_edge, second_edge| GeometricConstraintKindV1::Parallel {
                    first_edge,
                    second_edge,
                },
            ),
        ),
        ("point_on_line", point_on_line_case()),
        ("mirror_symmetry", mirror_case()),
        ("rotational_symmetry", rotation_case(60.0)),
        ("angle_bisector", angle_bisector_case()),
        (
            "length_ratio",
            two_edges(
                |numerator_edge, denominator_edge| GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio: 3.0,
                },
            ),
        ),
    ]
}

fn position_bits(pattern: &CreasePattern) -> BTreeMap<[u8; 16], (u64, u64)> {
    pattern
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.id.canonical_bytes(),
                (vertex.position.x.to_bits(), vertex.position.y.to_bits()),
            )
        })
        .collect()
}

#[test]
fn all_eleven_singleton_kinds_have_recertified_canonical_assignments() {
    for (name, case) in positive_cases() {
        let pattern_before = case.pattern.clone();
        let document_before = case.document.clone();
        let assignment =
            construct_single_constraint_exact_assignment_v1(&case.pattern, &case.document)
                .unwrap_or_else(|| panic!("{name} canonical template must certify"));
        assert_eq!(case.pattern, pattern_before, "{name}");
        assert_eq!(case.document, document_before, "{name}");
        assert_eq!(assignment.certificate().constraint_count(), 1, "{name}");
        assert!(!assignment.authorizes_project_mutation(), "{name}");
        assert!(!assignment.replayable_across_runtimes(), "{name}");
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                assignment.pattern(),
                &case.document,
            )
            .expect("constructed candidate remains valid")
            .is_some(),
            "{name}",
        );

        let mut reordered = case.pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        let reordered_assignment =
            construct_single_constraint_exact_assignment_v1(&reordered, &case.document)
                .unwrap_or_else(|| panic!("{name} reordered storage must certify"));
        assert_eq!(
            position_bits(assignment.pattern()),
            position_bits(reordered_assignment.pattern()),
            "{name}",
        );
    }
}

#[test]
fn invalid_degenerate_collapsing_and_subnormal_inputs_fail_closed() {
    let fixed = one_edge(|edge| GeometricConstraintKindV1::FixedLength {
        edge,
        length_mm: 3.0,
    });
    assert!(
        construct_single_constraint_exact_assignment_v1(
            &fixed.pattern,
            &document(std::iter::empty()),
        )
        .is_none(),
    );
    assert!(
        construct_single_constraint_exact_assignment_v1(
            &fixed.pattern,
            &document([
                fixed.document.constraints[0].clone(),
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixed.pattern.edges[0].id,
                }),
            ]),
        )
        .is_none(),
    );

    let mut degenerate = fixed.pattern.clone();
    degenerate.vertices[1].position = degenerate.vertices[0].position;
    assert!(
        construct_single_constraint_exact_assignment_v1(&degenerate, &fixed.document).is_none(),
    );

    let edge = fixed.pattern.edges[0].id;
    let nonfinite = document([record(GeometricConstraintKindV1::FixedLength {
        edge,
        length_mm: f64::NAN,
    })]);
    assert!(construct_single_constraint_exact_assignment_v1(&fixed.pattern, &nonfinite).is_none(),);

    let subnormal_rotation = rotation_case(f64::from_bits(1));
    assert!(
        construct_single_constraint_exact_assignment_v1(
            &subnormal_rotation.pattern,
            &subnormal_rotation.document,
        )
        .is_none(),
    );
    let subnormal_fixed_angle = fixed_angle_case(f64::from_bits(1));
    assert!(
        construct_single_constraint_exact_assignment_v1(
            &subnormal_fixed_angle.pattern,
            &subnormal_fixed_angle.document,
        )
        .is_none(),
    );

    let mut collapsing = FixtureBuilder::new();
    let start = collapsing.vertex();
    let end = collapsing.vertex();
    let target = collapsing.edge(start, end);
    for point in [
        Point2::new(3.0, 0.0),
        Point2::new(19.0, 32.0),
        Point2::new(-13.0, 8.0),
        Point2::new(1027.0, -512.0),
    ] {
        let blocker = collapsing.vertex();
        collapsing
            .vertices
            .last_mut()
            .expect("new blocker")
            .position = point;
        collapsing.edge(end, blocker);
    }
    let collapsing = collapsing.finish(GeometricConstraintKindV1::FixedLength {
        edge: target,
        length_mm: 3.0,
    });
    assert!(
        construct_single_constraint_exact_assignment_v1(&collapsing.pattern, &collapsing.document,)
            .is_none(),
        "every fixed translation collapses one unrelated incident edge",
    );
}

#[test]
fn modifying_a_returned_assignment_is_not_covered_by_its_certificate() {
    let case = one_edge(|edge| GeometricConstraintKindV1::FixedLength {
        edge,
        length_mm: 3.0,
    });
    let assignment = construct_single_constraint_exact_assignment_v1(&case.pattern, &case.document)
        .expect("fixed-length assignment");
    let mut modified = assignment.pattern().clone();
    modified.vertices[1].position.x = f64::from_bits(modified.vertices[1].position.x.to_bits() + 1);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&modified, &case.document)
            .expect("one-ULP modification remains structurally valid")
            .is_none(),
        "the opaque certificate cannot authorize a modified assignment",
    );
}
