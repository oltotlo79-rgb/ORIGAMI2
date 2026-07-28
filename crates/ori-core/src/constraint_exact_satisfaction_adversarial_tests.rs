use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};
use ori_numeric::{deterministic_atan2_v1, deterministic_sin_cos_degrees_v1};

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1, prepare_geometric_constraints_v1,
};

#[derive(Clone)]
struct SingleConstraintFixture {
    pattern: CreasePattern,
    document: GeometricConstraintDocumentV1,
}

impl SingleConstraintFixture {
    fn new(pattern: CreasePattern, constraint: GeometricConstraintKindV1) -> Self {
        Self {
            pattern,
            document: GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint,
                }],
            },
        }
    }

    fn vertex_mut(&mut self, id: VertexId) -> &mut Vertex {
        self.pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == id)
            .expect("fixture vertex")
    }

    fn constraint_mut(&mut self) -> &mut GeometricConstraintKindV1 {
        &mut self.document.constraints[0].constraint
    }
}

#[derive(Default)]
struct GeometryBuilder {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
}

impl GeometryBuilder {
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

    fn finish(self) -> CreasePattern {
        CreasePattern {
            vertices: self.vertices,
            edges: self.edges,
        }
    }
}

fn next_up(value: f64) -> f64 {
    assert!(value.is_finite() && value >= 0.0);
    f64::from_bits(value.to_bits() + 1)
}

fn fixed_length_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let start = geometry.vertex(0.0, 0.0);
    let end = geometry.vertex(2.0, 0.0);
    let edge = geometry.edge(start, end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(end).position.x = next_up(2.0);
    (positive, negative)
}

fn horizontal_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let start = geometry.vertex(0.0, 0.0);
    let end = geometry.vertex(1.0, 0.0);
    let edge = geometry.edge(start, end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::Horizontal { edge },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(end).position.y = f64::from_bits(1);
    (positive, negative)
}

fn vertical_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let start = geometry.vertex(0.0, 0.0);
    let end = geometry.vertex(0.0, 1.0);
    let edge = geometry.edge(start, end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::Vertical { edge },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(end).position.x = f64::from_bits(1);
    (positive, negative)
}

fn equal_length_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let first_start = geometry.vertex(0.0, 0.0);
    let first_end = geometry.vertex(1.0, 0.0);
    let second_start = geometry.vertex(0.0, 2.0);
    let second_end = geometry.vertex(1.0, 2.0);
    let first_edge = geometry.edge(first_start, first_end);
    let second_edge = geometry.edge(second_start, second_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(second_end).position.x = next_up(1.0);
    (positive, negative)
}

fn parallel_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let first_start = geometry.vertex(0.0, 0.0);
    let first_end = geometry.vertex(2.0, 0.0);
    let second_start = geometry.vertex(0.0, 1.0);
    let second_end = geometry.vertex(1.0, 1.0);
    let first_edge = geometry.edge(first_start, first_end);
    let second_edge = geometry.edge(second_start, second_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(second_end).position.y = next_up(1.0);
    (positive, negative)
}

fn point_on_line_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let line_start = geometry.vertex(0.0, 0.0);
    let line_end = geometry.vertex(2.0, 0.0);
    let point = geometry.vertex(1.0, 0.0);
    let line_edge = geometry.edge(line_start, line_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::PointOnLine {
            vertex: point,
            line_edge,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(point).position.y = f64::from_bits(1);
    (positive, negative)
}

fn length_ratio_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let origin = geometry.vertex(0.0, 0.0);
    let numerator_end = geometry.vertex(2.0, 0.0);
    let denominator_end = geometry.vertex(0.0, 1.0);
    let numerator_edge = geometry.edge(origin, numerator_end);
    let denominator_edge = geometry.edge(origin, denominator_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio: 2.0,
        },
    );
    let mut negative = positive.clone();
    let GeometricConstraintKindV1::LengthRatio { ratio, .. } = negative.constraint_mut() else {
        unreachable!("length-ratio fixture");
    };
    *ratio = next_up(2.0);
    (positive, negative)
}

fn fixed_angle_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let vertex = geometry.vertex(0.0, 0.0);
    let first_end = geometry.vertex(1.0, 0.0);
    let second_end = geometry.vertex(0.0, 1.0);
    let first_edge = geometry.edge(vertex, first_end);
    let second_edge = geometry.edge(vertex, second_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees: 90.0,
        },
    );
    let mut negative = positive.clone();
    let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } = negative.constraint_mut()
    else {
        unreachable!("fixed-angle fixture");
    };
    // One stored-degree ULP can disappear in the frozen degree conversion; use
    // a distinct admitted angle whose proof residual is necessarily non-zero.
    *angle_degrees = 89.0;
    (positive, negative)
}

fn mirror_symmetry_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let axis_start = geometry.vertex(-2.0, 0.0);
    let axis_end = geometry.vertex(2.0, 0.0);
    let first_vertex = geometry.vertex(0.0, 1.0);
    let second_vertex = geometry.vertex(0.0, -1.0);
    let axis_edge = geometry.edge(axis_start, axis_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(second_vertex).position.y = -next_up(1.0);
    (positive, negative)
}

fn rotational_symmetry_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let center_vertex = geometry.vertex(0.0, 0.0);
    let source_vertex = geometry.vertex(1.0, 0.0);
    // Derive the witness through the frozen proof kernel. The enclosing API
    // remains runtime-local until its model and wire metadata are migrated.
    let angle_degrees = 90.0_f64;
    let (target_y, target_x) = deterministic_sin_cos_degrees_v1(angle_degrees).unwrap();
    let target_vertex = geometry.vertex(target_x, target_y);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex,
            source_vertex,
            target_vertex,
            angle_degrees,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(target_vertex).position.x = f64::from_bits(
        target_x
            .to_bits()
            .checked_add(1)
            .expect("finite target bit"),
    );
    (positive, negative)
}

fn angle_bisector_case() -> (SingleConstraintFixture, SingleConstraintFixture) {
    let mut geometry = GeometryBuilder::default();
    let vertex = geometry.vertex(0.0, 0.0);
    let first_end = geometry.vertex(1.0, 0.0);
    let second_end = geometry.vertex(0.0, 1.0);
    let bisector_end = geometry.vertex(1.0, 1.0);
    let first_edge = geometry.edge(vertex, first_end);
    let second_edge = geometry.edge(vertex, second_end);
    let bisector_edge = geometry.edge(vertex, bisector_end);
    let positive = SingleConstraintFixture::new(
        geometry.finish(),
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        },
    );
    let mut negative = positive.clone();
    negative.vertex_mut(bisector_end).position.y = next_up(1.0);
    (positive, negative)
}

#[test]
fn each_constraint_kind_has_an_independent_positive_and_nonzero_residual_fixture() {
    let cases = [
        ("fixed_length", fixed_length_case()),
        ("fixed_angle", fixed_angle_case()),
        ("horizontal", horizontal_case()),
        ("vertical", vertical_case()),
        ("equal_length", equal_length_case()),
        ("parallel", parallel_case()),
        ("point_on_line", point_on_line_case()),
        ("length_ratio", length_ratio_case()),
        ("mirror_symmetry", mirror_symmetry_case()),
        ("rotational_symmetry", rotational_symmetry_case()),
        ("angle_bisector", angle_bisector_case()),
    ];

    for (name, (positive, negative)) in cases {
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                &positive.pattern,
                &positive.document,
            )
            .unwrap_or_else(|error| panic!("{name}: positive fixture failed validation: {error}"))
            .is_some(),
            "{name}: the independent exact-zero fixture must issue a certificate",
        );
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                &negative.pattern,
                &negative.document,
            )
            .unwrap_or_else(|error| panic!("{name}: negative fixture failed validation: {error}"))
            .is_none(),
            "{name}: its kind-specific nonzero residual must withdraw the certificate",
        );
    }
}

#[test]
fn fixed_angle_degree_one_ulp_follows_the_frozen_binary64_conversion() {
    let (positive, _) = fixed_angle_case();
    let mut aliased = positive.clone();
    let aliased_degrees = next_up(90.0);
    let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } = aliased.constraint_mut()
    else {
        unreachable!("fixed-angle fixture");
    };
    *angle_degrees = aliased_degrees;

    assert_ne!(90.0_f64.to_bits(), aliased_degrees.to_bits());
    let shared_residual_is_zero = crate::constraints::deterministic_fixed_angle_residual_binary64_v1(
        deterministic_atan2_v1(1.0, 0.0).unwrap(),
        aliased_degrees,
    ) == 0.0;
    let certificate_issued = certify_binary64_exact_geometric_constraint_satisfaction_v1(
        &aliased.pattern,
        &aliased.document,
    )
    .expect("the aliased document remains valid")
    .is_some();
    assert_eq!(
        certificate_issued, shared_residual_is_zero,
        "the certificate must follow the complete frozen proof residual, not stored-scalar or intermediate-conversion bits",
    );
}

#[test]
fn runtime_zero_angle_alias_is_not_misclassified_as_a_direct_conflict() {
    let (positive, _) = fixed_angle_case();
    let mut document = positive.document.clone();
    let mut aliased = document.constraints[0].clone();
    aliased.id = ConstraintId::new();
    let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } = &mut aliased.constraint
    else {
        unreachable!("fixed-angle fixture");
    };
    *angle_degrees = next_up(*angle_degrees);
    document.constraints.push(aliased);

    assert!(!matches!(
        prepare_geometric_constraints_v1(
            &positive.pattern,
            &document,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("the distinct finite fixed angles remain structurally valid")
        .preflight(),
        ConstraintPreflightV1::DirectConflict { .. },
    ));

    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&positive.pattern, &document);
    assert!(
        matches!(certificate, Ok(Some(_))),
        "the preflight and current-runtime certificate must agree on this rounded-zero angle alias",
    );
}
