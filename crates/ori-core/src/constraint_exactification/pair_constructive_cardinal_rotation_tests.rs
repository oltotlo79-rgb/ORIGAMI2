use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::construct_pair_constraint_exact_assignment_v1;

struct RotationFixture {
    pattern: CreasePattern,
    center: VertexId,
    source: VertexId,
    target: VertexId,
    source_radius: EdgeId,
    target_radius: EdgeId,
    source_target: EdgeId,
    unrelated: EdgeId,
}

fn fixture() -> RotationFixture {
    let center = VertexId::new();
    let source = VertexId::new();
    let target = VertexId::new();
    let unrelated_start = VertexId::new();
    let unrelated_end = VertexId::new();
    let source_radius = EdgeId::new();
    let target_radius = EdgeId::new();
    let source_target = EdgeId::new();
    let unrelated = EdgeId::new();
    RotationFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(8192.0, -4096.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(-17.0, 23.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(31.0, 47.0),
                },
                Vertex {
                    id: unrelated_start,
                    position: Point2::new(3.0, 5.0),
                },
                Vertex {
                    id: unrelated_end,
                    position: Point2::new(7.0, 11.0),
                },
            ],
            edges: vec![
                Edge {
                    id: source_radius,
                    start: center,
                    end: source,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: target_radius,
                    start: center,
                    end: target,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: source_target,
                    start: source,
                    end: target,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: unrelated,
                    start: unrelated_start,
                    end: unrelated_end,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        center,
        source,
        target,
        source_radius,
        target_radius,
        source_target,
        unrelated,
    }
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().map(record).collect(),
    }
}

fn rotation(fixture: &RotationFixture, angle_degrees: f64) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex: fixture.center,
        source_vertex: fixture.source,
        target_vertex: fixture.target,
        angle_degrees,
    }
}

fn fixed_radius(edge: EdgeId, length_mm: f64) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::FixedLength { edge, length_mm }
}

fn reverse_pattern_storage_and_edges(pattern: &CreasePattern) -> CreasePattern {
    let mut reversed = pattern.clone();
    reversed.vertices.reverse();
    reversed.edges.reverse();
    for edge in &mut reversed.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    reversed
}

#[test]
fn two_remaining_rotations_do_not_escape_as_degenerate_pattern_assignments() {
    let fixture = fixture();
    for angles in [[90.0, 180.0], [90.0, 270.0], [180.0, 270.0]] {
        for reversed in [false, true] {
            let pattern = if reversed {
                reverse_pattern_storage_and_edges(&fixture.pattern)
            } else {
                fixture.pattern.clone()
            };
            let mut document =
                document([rotation(&fixture, angles[0]), rotation(&fixture, angles[1])]);
            if reversed {
                document.constraints.reverse();
            }
            assert!(
                construct_pair_constraint_exact_assignment_v1(&pattern, &document).is_none(),
                "coincident rotation roles belong only to the residual-only algebraic path",
            );
        }
    }
}

#[test]
fn cardinal_rotation_and_either_fixed_radius_cover_the_full_finite_length_range() {
    let fixture = fixture();
    let lengths = [f64::from_bits(1), f64::MIN_POSITIVE, 1.0, f64::MAX];
    for angle_degrees in [90.0, 180.0, 270.0] {
        for radius in [fixture.source_radius, fixture.target_radius] {
            for length_mm in lengths {
                for reversed in [false, true] {
                    let pattern = if reversed {
                        reverse_pattern_storage_and_edges(&fixture.pattern)
                    } else {
                        fixture.pattern.clone()
                    };
                    let mut document = document([
                        rotation(&fixture, angle_degrees),
                        fixed_radius(radius, length_mm),
                    ]);
                    if reversed {
                        document.constraints.reverse();
                    }
                    let witness =
                        construct_pair_constraint_exact_assignment_v1(&pattern, &document)
                            .expect("the exact cardinal orbit satisfies the retained radius");
                    assert_eq!(witness.certificate().constraint_count(), 2);
                    assert_eq!(witness.certificate().equation_count(), 3);
                    assert!(witness.pattern().vertices.iter().all(|vertex| {
                        vertex.position.x.is_finite() && vertex.position.y.is_finite()
                    }));
                }
            }
        }
    }
}

#[test]
fn noncardinal_and_nonradius_pairs_remain_unavailable() {
    let fixture = fixture();
    for angle_degrees in [
        90.0_f64.next_down(),
        90.0_f64.next_up(),
        180.0_f64.next_down(),
        180.0_f64.next_up(),
        270.0_f64.next_down(),
        270.0_f64.next_up(),
        f64::from_bits(1),
    ] {
        let document = document([
            rotation(&fixture, angle_degrees),
            fixed_radius(fixture.source_radius, 1.0),
        ]);
        assert!(
            construct_pair_constraint_exact_assignment_v1(&fixture.pattern, &document).is_none(),
            "non-cardinal angle {angle_degrees:?} must not use the exact-cardinal template",
        );
    }

    for edge in [fixture.source_target, fixture.unrelated] {
        let document = document([rotation(&fixture, 90.0), fixed_radius(edge, 1.0)]);
        assert!(
            construct_pair_constraint_exact_assignment_v1(&fixture.pattern, &document).is_none(),
            "only center/source or center/target is a certified radius",
        );
    }
}

#[test]
fn invalid_cardinality_and_invalid_fixed_lengths_fail_closed() {
    let fixture = fixture();
    let single = document([rotation(&fixture, 90.0)]);
    assert!(construct_pair_constraint_exact_assignment_v1(&fixture.pattern, &single).is_none());

    for length_mm in [0.0, -0.0, -1.0, f64::INFINITY, f64::NAN] {
        let document = document([
            rotation(&fixture, 90.0),
            fixed_radius(fixture.source_radius, length_mm),
        ]);
        assert!(
            construct_pair_constraint_exact_assignment_v1(&fixture.pattern, &document).is_none()
        );
    }
}
