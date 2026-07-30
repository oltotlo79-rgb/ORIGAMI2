use std::collections::BTreeMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::{
    MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    construct_bounded_singleton_composition_exact_assignment_v1,
    construct_four_constraint_exact_assignment_v1, construct_single_constraint_exact_assignment_v1,
    construct_three_constraint_exact_assignment_v1, construct_two_constraint_exact_assignment_v1,
};
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
        assert_eq!(
            assignment.transcendental_model_id(),
            ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            "{name}",
        );
        assert_eq!(
            assignment.replayable_across_runtimes(),
            ori_numeric::deterministic_transcendental_model_supported_v1(),
            "{name}",
        );
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

#[test]
fn two_vertex_disjoint_singleton_assignments_are_merged_and_recertified() {
    let mut builder = FixtureBuilder::new();
    let first_start = builder.vertex();
    let first_end = builder.vertex();
    let second_start = builder.vertex();
    let second_end = builder.vertex();
    let first = builder.edge(first_start, first_end);
    let second = builder.edge(second_start, second_end);
    let pattern = CreasePattern {
        vertices: builder.vertices,
        edges: builder.edges,
    };
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second,
            length_mm: 3.0,
        }),
    ]);
    let pattern_before = pattern.clone();
    let source_before = source.clone();

    let assignment = construct_two_constraint_exact_assignment_v1(&pattern, &source)
        .expect("vertex-disjoint singleton assignments must compose");
    assert_eq!(pattern, pattern_before);
    assert_eq!(source, source_before);
    assert_eq!(assignment.certificate().constraint_count(), 2);
    assert_eq!(assignment.certificate().equation_count(), 2);
    assert!(!assignment.authorizes_project_mutation());
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source,)
            .expect("the composed pattern remains structurally valid")
            .is_some(),
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let mut reordered_source = source.clone();
    reordered_source.constraints.reverse();
    let reordered =
        construct_two_constraint_exact_assignment_v1(&reordered_pattern, &reordered_source)
            .expect("storage and document order must not change composition");
    assert_eq!(
        position_bits(assignment.pattern()),
        position_bits(reordered.pattern()),
    );
}

#[test]
fn two_record_pair_template_precedes_incompatible_singleton_coordinates() {
    let case = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
    let edge = case.pattern.edges[0].id;
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
    ]);

    // The independent singleton templates place this edge at lengths two and
    // one respectively, so their shared endpoint coordinates cannot merge.
    // The bounded pair template resolves both records together and still earns
    // authority only through the complete residual verifier.
    let exact_two = construct_two_constraint_exact_assignment_v1(&case.pattern, &source)
        .expect("the wider bounded pair template must construct");
    let bounded =
        construct_bounded_singleton_composition_exact_assignment_v1(&case.pattern, &source)
            .expect("the public bounded compositor must expose the pair prepass");
    assert_eq!(
        position_bits(exact_two.pattern()),
        position_bits(bounded.pattern()),
    );
    assert_eq!(bounded.certificate().constraint_count(), 2);
    assert!(!bounded.authorizes_project_mutation());
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(bounded.pattern(), &source,)
            .expect("the pair candidate remains structurally valid")
            .is_some(),
    );
}

#[test]
fn every_singleton_kind_composes_with_a_vertex_disjoint_record() {
    for (name, mut case) in positive_cases() {
        let companion = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
        case.pattern.vertices.extend(companion.pattern.vertices);
        case.pattern.edges.extend(companion.pattern.edges);
        case.document
            .constraints
            .push(companion.document.constraints[0].clone());

        let assignment =
            construct_two_constraint_exact_assignment_v1(&case.pattern, &case.document)
                .unwrap_or_else(|| panic!("{name} must compose across vertex-disjoint components"));
        assert_eq!(assignment.certificate().constraint_count(), 2, "{name}");
        assert!(!assignment.authorizes_project_mutation(), "{name}");
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                assignment.pattern(),
                &case.document,
            )
            .expect("the composed full document remains valid")
            .is_some(),
            "{name}",
        );
    }
}

#[test]
fn every_singleton_kind_composes_in_bounded_three_through_sixteen_record_documents() {
    for (name, mut case) in positive_cases() {
        let horizontal = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
        let vertical = one_edge(|edge| GeometricConstraintKindV1::Vertical { edge });
        for companion in [horizontal, vertical] {
            case.pattern.vertices.extend(companion.pattern.vertices);
            case.pattern.edges.extend(companion.pattern.edges);
            case.document
                .constraints
                .push(companion.document.constraints[0].clone());
        }

        let assignment =
            construct_three_constraint_exact_assignment_v1(&case.pattern, &case.document)
                .unwrap_or_else(|| panic!("{name} must compose with two vertex-disjoint records"));
        assert_eq!(assignment.certificate().constraint_count(), 3, "{name}");
        assert!(!assignment.authorizes_project_mutation(), "{name}");
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                assignment.pattern(),
                &case.document,
            )
            .expect("the three-record full document remains valid")
            .is_some(),
            "{name}",
        );

        let mut reordered_pattern = case.pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        let mut reordered_document = case.document.clone();
        reordered_document.constraints.reverse();
        let reordered =
            construct_three_constraint_exact_assignment_v1(&reordered_pattern, &reordered_document)
                .unwrap_or_else(|| panic!("{name} reordered three-record composition"));
        assert_eq!(
            position_bits(assignment.pattern()),
            position_bits(reordered.pattern()),
            "{name}",
        );

        let fixed = one_edge(|edge| GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        });
        case.pattern.vertices.extend(fixed.pattern.vertices);
        case.pattern.edges.extend(fixed.pattern.edges);
        case.document
            .constraints
            .push(fixed.document.constraints[0].clone());

        let four_assignment =
            construct_four_constraint_exact_assignment_v1(&case.pattern, &case.document)
                .unwrap_or_else(|| {
                    panic!("{name} must compose with three vertex-disjoint records")
                });
        assert_eq!(
            four_assignment.certificate().constraint_count(),
            4,
            "{name}"
        );
        assert!(!four_assignment.authorizes_project_mutation(), "{name}");
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                four_assignment.pattern(),
                &case.document,
            )
            .expect("the four-record full document remains valid")
            .is_some(),
            "{name}",
        );

        let mut boundary_assignment = four_assignment;
        for expected_count in 5..=MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
            let companion = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
            case.pattern.vertices.extend(companion.pattern.vertices);
            case.pattern.edges.extend(companion.pattern.edges);
            case.document
                .constraints
                .push(companion.document.constraints[0].clone());
            boundary_assignment = construct_bounded_singleton_composition_exact_assignment_v1(
                &case.pattern,
                &case.document,
            )
            .unwrap_or_else(|| panic!("{name} must compose at bounded count {expected_count}"));
            assert_eq!(
                boundary_assignment.certificate().constraint_count(),
                expected_count,
                "{name}",
            );
            assert!(!boundary_assignment.authorizes_project_mutation(), "{name}");
            assert!(
                certify_binary64_exact_geometric_constraint_satisfaction_v1(
                    boundary_assignment.pattern(),
                    &case.document,
                )
                .expect("the bounded full document remains valid")
                .is_some(),
                "{name} count {expected_count}",
            );
        }

        let mut reordered_pattern = case.pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        let mut reordered_document = case.document.clone();
        reordered_document.constraints.reverse();
        let reordered = construct_bounded_singleton_composition_exact_assignment_v1(
            &reordered_pattern,
            &reordered_document,
        )
        .unwrap_or_else(|| panic!("{name} reordered sixteen-record composition"));
        assert_eq!(
            position_bits(boundary_assignment.pattern()),
            position_bits(reordered.pattern()),
            "{name}",
        );
    }
}

#[test]
fn shared_vertices_require_bit_identical_singleton_assignments() {
    let mut compatible = FixtureBuilder::new();
    let shared = compatible.vertex();
    let first_end = compatible.vertex();
    let second_end = compatible.vertex();
    let first = compatible.edge(shared, first_end);
    let second = compatible.edge(shared, second_end);
    let compatible_pattern = CreasePattern {
        vertices: compatible.vertices,
        edges: compatible.edges,
    };
    let compatible_document = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);
    assert!(
        construct_two_constraint_exact_assignment_v1(&compatible_pattern, &compatible_document,)
            .is_some(),
        "bit-identical assignments for the same two endpoints may compose",
    );

    let mut conflicting = FixtureBuilder::new();
    let first_start = conflicting.vertex();
    let shared = conflicting.vertex();
    let second_end = conflicting.vertex();
    let first = conflicting.edge(first_start, shared);
    let second = conflicting.edge(shared, second_end);
    let conflicting_pattern = CreasePattern {
        vertices: conflicting.vertices,
        edges: conflicting.edges,
    };
    let conflicting_document = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);
    assert!(
        construct_two_constraint_exact_assignment_v1(&conflicting_pattern, &conflicting_document,)
            .is_none(),
        "a shared vertex assigned different coordinate bits must fail closed",
    );
}

#[test]
fn bounded_shared_assignments_must_all_be_bit_identical_through_sixteen_records() {
    let mut compatible = FixtureBuilder::new();
    let start = compatible.vertex();
    let end = compatible.vertex();
    let edge = compatible.edge(start, end);
    let compatible_pattern = CreasePattern {
        vertices: compatible.vertices,
        edges: compatible.edges,
    };
    let mut compatible_document = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
    ]);
    let compatible_assignment =
        construct_three_constraint_exact_assignment_v1(&compatible_pattern, &compatible_document)
            .expect("three bit-identical endpoint assignments may compose");
    assert_eq!(compatible_assignment.certificate().constraint_count(), 3,);
    compatible_document
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    let compatible_assignment =
        construct_four_constraint_exact_assignment_v1(&compatible_pattern, &compatible_document)
            .expect("four bit-identical endpoint assignments may compose");
    assert_eq!(compatible_assignment.certificate().constraint_count(), 4);
    while compatible_document.constraints.len() < MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        compatible_document
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    let compatible_assignment = construct_bounded_singleton_composition_exact_assignment_v1(
        &compatible_pattern,
        &compatible_document,
    )
    .expect("sixteen bit-identical endpoint assignments may compose");
    assert_eq!(
        compatible_assignment.certificate().constraint_count(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );

    let mut conflicting = FixtureBuilder::new();
    let first_start = conflicting.vertex();
    let shared = conflicting.vertex();
    let second_end = conflicting.vertex();
    let detached_start = conflicting.vertex();
    let detached_end = conflicting.vertex();
    let first = conflicting.edge(first_start, shared);
    let second = conflicting.edge(shared, second_end);
    let detached = conflicting.edge(detached_start, detached_end);
    let conflicting_pattern = CreasePattern {
        vertices: conflicting.vertices,
        edges: conflicting.edges,
    };
    let mut conflicting_document = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: detached,
            length_mm: 1.0,
        }),
    ]);
    assert!(
        construct_three_constraint_exact_assignment_v1(
            &conflicting_pattern,
            &conflicting_document,
        )
        .is_none(),
        "one shared-coordinate disagreement must reject the full composition",
    );
    conflicting_document
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal {
            edge: detached,
        }));
    assert!(
        construct_four_constraint_exact_assignment_v1(&conflicting_pattern, &conflicting_document,)
            .is_none(),
        "one shared-coordinate disagreement must reject all four records",
    );
    while conflicting_document.constraints.len() < MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
    {
        conflicting_document
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal {
                edge: detached,
            }));
    }
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &conflicting_pattern,
            &conflicting_document,
        )
        .is_none(),
        "one shared-coordinate disagreement must reject the sixteen-record boundary",
    );
}

#[test]
fn two_constraint_composition_is_bounded_to_exactly_two_nondirect_records() {
    let case = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
    assert!(construct_two_constraint_exact_assignment_v1(&case.pattern, &case.document).is_none(),);

    let edge = case.pattern.edges[0].id;
    let incompatible = document([
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Vertical { edge }),
    ]);
    assert!(construct_two_constraint_exact_assignment_v1(&case.pattern, &incompatible).is_none(),);

    let direct = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
    ]);
    assert!(construct_two_constraint_exact_assignment_v1(&case.pattern, &direct).is_none(),);
}

#[test]
fn bounded_composition_accepts_two_through_sixteen_and_rejects_seventeen_or_direct() {
    assert_eq!(MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1, 16);
    let case = one_edge(|edge| GeometricConstraintKindV1::Horizontal { edge });
    assert!(
        construct_three_constraint_exact_assignment_v1(&case.pattern, &case.document).is_none(),
    );
    assert!(construct_four_constraint_exact_assignment_v1(&case.pattern, &case.document).is_none(),);
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&case.pattern, &case.document,)
            .is_none(),
    );

    let edge = case.pattern.edges[0].id;
    let direct = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
    ]);
    assert!(construct_three_constraint_exact_assignment_v1(&case.pattern, &direct).is_none(),);
    assert!(construct_four_constraint_exact_assignment_v1(&case.pattern, &direct).is_none(),);

    let mut four_records = document([
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
    ]);
    assert!(construct_three_constraint_exact_assignment_v1(&case.pattern, &four_records).is_none(),);
    assert!(construct_four_constraint_exact_assignment_v1(&case.pattern, &four_records).is_some(),);

    four_records
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    assert!(construct_four_constraint_exact_assignment_v1(&case.pattern, &four_records).is_none(),);
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&case.pattern, &four_records,)
            .is_some(),
    );
    while four_records.constraints.len() < MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        four_records
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal { edge }));
        assert!(
            construct_bounded_singleton_composition_exact_assignment_v1(
                &case.pattern,
                &four_records,
            )
            .is_some(),
        );
    }
    four_records
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&case.pattern, &four_records,)
            .is_none(),
        "seventeen records must remain outside the bounded constructor",
    );

    let mut direct_at_boundary = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
        record(GeometricConstraintKindV1::Horizontal { edge }),
    ]);
    while direct_at_boundary.constraints.len() < MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        direct_at_boundary
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &case.pattern,
            &direct_at_boundary,
        )
        .is_none(),
    );
}
