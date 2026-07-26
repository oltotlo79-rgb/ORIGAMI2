use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use crate::{ConstraintSolveErrorV1, certify_binary64_exact_geometric_constraint_satisfaction_v1};

#[derive(Default)]
struct ExactFixture {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
    constraints: Vec<GeometricConstraintRecordV1>,
}

impl ExactFixture {
    fn vertex(&mut self, x: f64, y: f64) -> VertexId {
        let id = VertexId::new();
        self.vertices.push(Vertex {
            id,
            position: Point2::new(x, y),
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

    fn constraint(&mut self, constraint: GeometricConstraintKindV1) {
        self.constraints.push(GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint,
        });
    }

    fn finish(self) -> (CreasePattern, GeometricConstraintDocumentV1) {
        (
            CreasePattern {
                vertices: self.vertices,
                edges: self.edges,
            },
            GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: self.constraints,
            },
        )
    }
}

fn all_constraint_kinds_exact_fixture() -> (CreasePattern, GeometricConstraintDocumentV1) {
    let mut fixture = ExactFixture::default();

    let origin = fixture.vertex(0.0, 0.0);
    let x_one = fixture.vertex(1.0, 0.0);
    let x_two = fixture.vertex(2.0, 0.0);
    let y_one = fixture.vertex(0.0, 1.0);
    let y_two = fixture.vertex(0.0, 2.0);
    let diagonal = fixture.vertex(1.0, 1.0);
    let below = fixture.vertex(0.0, -1.0);
    let rotation_angle_degrees = 90.0_f64;
    let rotation_angle_radians = rotation_angle_degrees.to_radians();
    let rotated_x_one = fixture.vertex(rotation_angle_radians.cos(), rotation_angle_radians.sin());

    let horizontal_one = fixture.edge(origin, x_one);
    let horizontal_two = fixture.edge(origin, x_two);
    let vertical_one = fixture.edge(origin, y_one);
    let vertical_two = fixture.edge(origin, y_two);
    let diagonal_edge = fixture.edge(origin, diagonal);
    let parallel_offset_start = fixture.vertex(0.0, 3.0);
    let parallel_offset_end = fixture.vertex(2.0, 3.0);
    let parallel_offset = fixture.edge(parallel_offset_start, parallel_offset_end);
    let mirror_axis = horizontal_two;

    fixture.constraint(GeometricConstraintKindV1::FixedLength {
        edge: horizontal_two,
        length_mm: 2.0,
    });
    fixture.constraint(GeometricConstraintKindV1::Horizontal {
        edge: horizontal_one,
    });
    fixture.constraint(GeometricConstraintKindV1::Vertical { edge: vertical_one });
    fixture.constraint(GeometricConstraintKindV1::EqualLength {
        first_edge: horizontal_one,
        second_edge: vertical_one,
    });
    fixture.constraint(GeometricConstraintKindV1::Parallel {
        first_edge: horizontal_two,
        second_edge: parallel_offset,
    });
    fixture.constraint(GeometricConstraintKindV1::PointOnLine {
        vertex: x_one,
        line_edge: horizontal_two,
    });
    fixture.constraint(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: horizontal_two,
        denominator_edge: horizontal_one,
        ratio: 2.0,
    });
    fixture.constraint(GeometricConstraintKindV1::FixedAngle {
        vertex: origin,
        first_edge: horizontal_one,
        second_edge: horizontal_two,
        angle_degrees: 0.0,
    });
    fixture.constraint(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: y_one,
        second_vertex: below,
        axis_edge: mirror_axis,
    });
    fixture.constraint(GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex: origin,
        source_vertex: x_one,
        target_vertex: rotated_x_one,
        angle_degrees: rotation_angle_degrees,
    });
    fixture.constraint(GeometricConstraintKindV1::AngleBisector {
        vertex: origin,
        first_edge: horizontal_one,
        second_edge: vertical_one,
        bisector_edge: diagonal_edge,
    });

    // Keep otherwise unused geometry present to exercise canonical validation.
    let _ = (x_two, y_two, vertical_two);
    fixture.finish()
}

#[test]
fn exact_witness_covers_all_eleven_constraint_kinds() {
    let (pattern, document) = all_constraint_kinds_exact_fixture();
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document)
            .expect("valid exact assignment")
            .expect("every production residual is exactly zero");

    assert_eq!(certificate.constraint_count(), 11);
    assert_eq!(certificate.equation_count(), 14);
}

#[test]
fn any_finite_nonzero_residual_withdraws_the_positive_certificate() {
    let (mut pattern, document) = all_constraint_kinds_exact_fixture();
    let horizontal = document
        .constraints
        .iter()
        .find_map(|record| match record.constraint {
            GeometricConstraintKindV1::Horizontal { edge } => Some(edge),
            _ => None,
        })
        .expect("horizontal constraint");
    let endpoint = pattern
        .edges
        .iter()
        .find(|edge| edge.id == horizontal)
        .expect("horizontal edge")
        .end;
    pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == endpoint)
        .expect("horizontal endpoint")
        .position
        .y = f64::from_bits(1);

    assert_eq!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document)
            .expect("finite nonzero residual is a normal unknown result"),
        None
    );
}

#[test]
fn degenerate_normalized_geometry_fails_closed() {
    let (mut pattern, document) = all_constraint_kinds_exact_fixture();
    let axis = document
        .constraints
        .iter()
        .find_map(|record| match record.constraint {
            GeometricConstraintKindV1::MirrorSymmetry { axis_edge, .. } => Some(axis_edge),
            _ => None,
        })
        .expect("mirror axis");
    let edge = pattern
        .edges
        .iter()
        .find(|edge| edge.id == axis)
        .expect("axis edge")
        .clone();
    let start = pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == edge.start)
        .expect("axis start")
        .position;
    pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == edge.end)
        .expect("axis end")
        .position = start;

    assert_eq!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document),
        Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
    );
}

#[test]
fn certificate_counts_are_invariant_to_storage_order() {
    let (pattern, document) = all_constraint_kinds_exact_fixture();
    let expected = certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document)
        .expect("ordered fixture");
    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let mut reordered_document = document.clone();
    reordered_document.constraints.reverse();

    assert_eq!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &reordered_pattern,
            &reordered_document,
        )
        .expect("reordered fixture"),
        expected
    );
}

#[test]
fn invalid_document_cannot_produce_a_certificate() {
    let (pattern, mut document) = all_constraint_kinds_exact_fixture();
    document.schema_version += 1;

    assert_eq!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document),
        Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
    );
}
