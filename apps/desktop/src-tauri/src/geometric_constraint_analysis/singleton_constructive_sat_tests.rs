use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ori_domain::{
    ConstraintId, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::*;

struct SingletonCase {
    name: &'static str,
    pattern: CreasePattern,
    document: GeometricConstraintDocumentV1,
    equation_count: usize,
}

#[derive(Default)]
struct FixtureBuilder {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
}

impl FixtureBuilder {
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

    fn finish(
        self,
        name: &'static str,
        constraint: GeometricConstraintKindV1,
        equation_count: usize,
    ) -> SingletonCase {
        SingletonCase {
            name,
            pattern: CreasePattern {
                vertices: self.vertices,
                edges: self.edges,
            },
            document: document([constraint]),
            equation_count,
        }
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

fn one_edge_case(
    name: &'static str,
    constraint: impl FnOnce(EdgeId) -> GeometricConstraintKindV1,
) -> SingletonCase {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(0.0, 0.0);
    let end = fixture.vertex(3.0, 4.0);
    let edge = fixture.edge(start, end);
    fixture.finish(name, constraint(edge), 1)
}

fn two_edge_case(
    name: &'static str,
    constraint: impl FnOnce(EdgeId, EdgeId) -> GeometricConstraintKindV1,
) -> SingletonCase {
    let mut fixture = FixtureBuilder::default();
    let center = fixture.vertex(0.0, 0.0);
    let first_end = fixture.vertex(1.0, 0.0);
    let second_end = fixture.vertex(0.0, 2.0);
    let first = fixture.edge(center, first_end);
    let second = fixture.edge(center, second_end);
    fixture.finish(name, constraint(first, second), 1)
}

fn singleton_cases() -> Vec<SingletonCase> {
    let mut fixed_angle = FixtureBuilder::default();
    let angle_center = fixed_angle.vertex(0.0, 0.0);
    let angle_first_end = fixed_angle.vertex(1.0, 0.0);
    let angle_second_end = fixed_angle.vertex(0.0, 1.0);
    let angle_first = fixed_angle.edge(angle_center, angle_first_end);
    let angle_second = fixed_angle.edge(angle_center, angle_second_end);

    let mut point_on_line = FixtureBuilder::default();
    let line_start = point_on_line.vertex(0.0, 0.0);
    let line_end = point_on_line.vertex(2.0, 0.0);
    let off_line = point_on_line.vertex(1.0, 1.0);
    let line = point_on_line.edge(line_start, line_end);

    let mut mirror = FixtureBuilder::default();
    let axis_start = mirror.vertex(-2.0, 0.0);
    let axis_end = mirror.vertex(2.0, 0.0);
    let mirror_first = mirror.vertex(0.0, 1.0);
    let mirror_second = mirror.vertex(1.0, -1.0);
    let axis = mirror.edge(axis_start, axis_end);

    let mut rotation = FixtureBuilder::default();
    let rotation_center = rotation.vertex(0.0, 0.0);
    let rotation_source = rotation.vertex(1.0, 0.0);
    let wrong_rotation_target = rotation.vertex(1.0, 1.0);

    let mut bisector = FixtureBuilder::default();
    let bisector_center = bisector.vertex(0.0, 0.0);
    let bisector_first_end = bisector.vertex(1.0, 0.0);
    let bisector_second_end = bisector.vertex(0.0, 1.0);
    let wrong_bisector_end = bisector.vertex(1.0, 0.5);
    let bisector_first = bisector.edge(bisector_center, bisector_first_end);
    let bisector_second = bisector.edge(bisector_center, bisector_second_end);
    let wrong_bisector = bisector.edge(bisector_center, wrong_bisector_end);

    vec![
        one_edge_case("fixed_length", |edge| {
            GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 3.0,
            }
        }),
        fixed_angle.finish(
            "fixed_angle",
            GeometricConstraintKindV1::FixedAngle {
                vertex: angle_center,
                first_edge: angle_first,
                second_edge: angle_second,
                angle_degrees: 60.0,
            },
            1,
        ),
        one_edge_case("horizontal", |edge| GeometricConstraintKindV1::Horizontal {
            edge,
        }),
        one_edge_case("vertical", |edge| GeometricConstraintKindV1::Vertical {
            edge,
        }),
        two_edge_case("equal_length", |first_edge, second_edge| {
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
        }),
        two_edge_case("parallel", |first_edge, second_edge| {
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            }
        }),
        point_on_line.finish(
            "point_on_line",
            GeometricConstraintKindV1::PointOnLine {
                vertex: off_line,
                line_edge: line,
            },
            1,
        ),
        mirror.finish(
            "mirror_symmetry",
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: mirror_first,
                second_vertex: mirror_second,
                axis_edge: axis,
            },
            2,
        ),
        rotation.finish(
            "rotational_symmetry",
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: rotation_center,
                source_vertex: rotation_source,
                target_vertex: wrong_rotation_target,
                angle_degrees: 90.0,
            },
            2,
        ),
        bisector.finish(
            "angle_bisector",
            GeometricConstraintKindV1::AngleBisector {
                vertex: bisector_center,
                first_edge: bisector_first,
                second_edge: bisector_second,
                bisector_edge: wrong_bisector,
            },
            2,
        ),
        two_edge_case("length_ratio", |numerator_edge, denominator_edge| {
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio: 3.0,
            }
        }),
    ]
}

fn expected_positive(equation_count: usize) -> GeometricConstraintPreflightResult {
    expected_positive_document(1, equation_count)
}

fn expected_positive_document(
    constraint_count: usize,
    equation_count: usize,
) -> GeometricConstraintPreflightResult {
    GeometricConstraintPreflightResult::ProvenSatisfiable {
        model_id: ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
        transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        evidence_kind: GeometricConstraintSatisfactionEvidenceKind::DetachedConstructedAssignment,
        constraint_count,
        equation_count,
        authorizes_project_mutation: false,
        replayable_across_runtimes: ori_numeric::deterministic_transcendental_model_supported_v1(),
    }
}

#[test]
fn all_eleven_unsatisfied_singleton_kinds_publish_only_a_constructive_sat_observation() {
    for case in singleton_cases() {
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                &case.pattern,
                &case.document,
            )
            .expect("singleton source must be structurally valid")
            .is_none(),
            "{} source must not already satisfy its constraint",
            case.name,
        );
        let source_before = case.pattern.clone();
        let document_before = case.document.clone();

        assert_eq!(
            analyze_geometric_constraint_document(&case.pattern, &case.document),
            expected_positive(case.equation_count),
            "{}",
            case.name,
        );
        assert_eq!(case.pattern, source_before, "{}", case.name);
        assert_eq!(case.document, document_before, "{}", case.name);
    }
}

#[test]
fn bounded_two_record_pair_templates_publish_only_detached_constructive_sat() {
    let mut disjoint = FixtureBuilder::default();
    let first_start = disjoint.vertex(0.0, 0.0);
    let first_end = disjoint.vertex(3.0, 4.0);
    let second_start = disjoint.vertex(8.0, 8.0);
    let second_end = disjoint.vertex(11.0, 12.0);
    let first = disjoint.edge(first_start, first_end);
    let second = disjoint.edge(second_start, second_end);
    let disjoint_pattern = CreasePattern {
        vertices: disjoint.vertices,
        edges: disjoint.edges,
    };
    let disjoint_document = document([
        GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        },
        GeometricConstraintKindV1::FixedLength {
            edge: second,
            length_mm: 3.0,
        },
    ]);
    let disjoint_before = disjoint_pattern.clone();
    let disjoint_document_before = disjoint_document.clone();
    assert_eq!(
        analyze_geometric_constraint_document(&disjoint_pattern, &disjoint_document),
        expected_positive_document(2, 2),
    );
    assert_eq!(disjoint_pattern, disjoint_before);
    assert_eq!(disjoint_document, disjoint_document_before);

    let mut shared = FixtureBuilder::default();
    let start = shared.vertex(0.0, 0.0);
    let end = shared.vertex(3.0, 4.0);
    let edge = shared.edge(start, end);
    let shared_pattern = CreasePattern {
        vertices: shared.vertices,
        edges: shared.edges,
    };
    let shared_document = document([
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        },
        GeometricConstraintKindV1::Horizontal { edge },
    ]);
    let shared_result = analyze_geometric_constraint_document(&shared_pattern, &shared_document);
    assert_eq!(shared_result, expected_positive_document(2, 2));
    let encoded =
        serde_json::to_value(&shared_result).expect("serialize two-record constructive SAT");
    let object = encoded
        .as_object()
        .expect("the tagged SAT response is an object");
    assert_eq!(object.len(), 8);
    assert_eq!(
        object.get("status"),
        Some(&serde_json::json!("proven_satisfiable")),
    );
    assert_eq!(
        object.get("evidence_kind"),
        Some(&serde_json::json!("detached_constructed_assignment")),
    );
    for forbidden in ["pattern", "vertices", "positions", "assignment"] {
        assert!(
            !object.contains_key(forbidden),
            "constructed coordinates must not cross the DTO as {forbidden}",
        );
    }
}

#[test]
fn bounded_three_through_sixteen_record_constructive_sat_requires_compatible_singletons() {
    let mut compatible = FixtureBuilder::default();
    let start = compatible.vertex(0.0, 0.0);
    let end = compatible.vertex(3.0, 4.0);
    let edge = compatible.edge(start, end);
    let compatible_pattern = CreasePattern {
        vertices: compatible.vertices,
        edges: compatible.edges,
    };
    let mut compatible_document = document([
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        },
        GeometricConstraintKindV1::Horizontal { edge },
        GeometricConstraintKindV1::Horizontal { edge },
    ]);
    let compatible_before = compatible_pattern.clone();
    for expected_count in 3..=ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        while compatible_document.constraints.len() < expected_count {
            compatible_document
                .constraints
                .push(record(GeometricConstraintKindV1::Horizontal { edge }));
        }
        let compatible_document_before = compatible_document.clone();
        assert_eq!(
            analyze_geometric_constraint_document(&compatible_pattern, &compatible_document),
            expected_positive_document(expected_count, expected_count),
        );
        assert_eq!(compatible_pattern, compatible_before);
        assert_eq!(compatible_document, compatible_document_before);
    }

    let compatible_result =
        analyze_geometric_constraint_document(&compatible_pattern, &compatible_document);
    let encoded = serde_json::to_value(&compatible_result)
        .expect("serialize sixteen-record constructive SAT");
    let object = encoded
        .as_object()
        .expect("the tagged SAT response is an object");
    assert_eq!(object.len(), 8);
    assert_eq!(
        object.get("evidence_kind"),
        Some(&serde_json::json!("detached_constructed_assignment")),
    );
    for forbidden in ["pattern", "vertices", "positions", "assignment"] {
        assert!(
            !object.contains_key(forbidden),
            "constructed coordinates must not cross the DTO as {forbidden}",
        );
    }
    let mut seventeen_records = compatible_document.clone();
    seventeen_records
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    assert_eq!(
        analyze_geometric_constraint_document(&compatible_pattern, &seventeen_records),
        GeometricConstraintPreflightResult::NoDirectConflict,
        "seventeen records remain outside the bounded constructor",
    );

    let mut conflicting = FixtureBuilder::default();
    let first_start = conflicting.vertex(0.0, 0.0);
    let shared = conflicting.vertex(1.0, 1.0);
    let second_end = conflicting.vertex(2.0, 2.0);
    let detached_start = conflicting.vertex(8.0, 8.0);
    let detached_end = conflicting.vertex(11.0, 12.0);
    let first = conflicting.edge(first_start, shared);
    let second = conflicting.edge(shared, second_end);
    let detached = conflicting.edge(detached_start, detached_end);
    let conflicting_pattern = CreasePattern {
        vertices: conflicting.vertices,
        edges: conflicting.edges,
    };
    let mut conflicting_document = document([
        GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        },
        GeometricConstraintKindV1::Vertical { edge: second },
        GeometricConstraintKindV1::FixedLength {
            edge: detached,
            length_mm: 1.0,
        },
    ]);
    assert_eq!(
        analyze_geometric_constraint_document(&conflicting_pattern, &conflicting_document),
        GeometricConstraintPreflightResult::NoDirectConflict,
    );
    while conflicting_document.constraints.len()
        < ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
    {
        conflicting_document
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal {
                edge: detached,
            }));
    }
    assert_eq!(
        analyze_geometric_constraint_document(&conflicting_pattern, &conflicting_document),
        GeometricConstraintPreflightResult::NoDirectConflict,
    );
}

#[test]
fn direct_conflicts_keep_priority_and_conflicting_two_record_composition_falls_through() {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(0.0, 0.0);
    let end = fixture.vertex(1.0, 1.0);
    let edge = fixture.edge(start, end);
    let pattern = CreasePattern {
        vertices: fixture.vertices,
        edges: fixture.edges,
    };

    let two_records = document([
        GeometricConstraintKindV1::Horizontal { edge },
        GeometricConstraintKindV1::Vertical { edge },
    ]);
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &two_records),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len() == 2
    ));

    let two_record_direct = document([
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        },
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        },
    ]);
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &two_record_direct),
        GeometricConstraintPreflightResult::DirectConflict {
            ref conflicts,
            ..
        } if conflicts.len() == 1
    ));

    let mut direct = document([
        GeometricConstraintKindV1::Horizontal { edge },
        GeometricConstraintKindV1::Vertical { edge },
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        },
    ]);
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &direct),
        GeometricConstraintPreflightResult::DirectConflict {
            ref conflicts,
            ..
        } if conflicts.len() == 1
    ));
    direct
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &direct),
        GeometricConstraintPreflightResult::DirectConflict {
            ref conflicts,
            ..
        } if conflicts.len() == 1
    ));
    while direct.constraints.len() < ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        direct
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &direct),
        GeometricConstraintPreflightResult::DirectConflict {
            ref conflicts,
            ..
        } if conflicts.len() == 1
    ));
    direct
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &direct),
        GeometricConstraintPreflightResult::DirectConflict {
            ref conflicts,
            ..
        } if conflicts.len() == 1
    ));
}

fn four_translation_collapse_case() -> SingletonCase {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(100.0, 100.0);
    let end = fixture.vertex(101.0, 100.0);
    let target = fixture.edge(start, end);
    for point in [
        Point2::new(3.0, 0.0),
        Point2::new(19.0, 32.0),
        Point2::new(-13.0, 8.0),
        Point2::new(1027.0, -512.0),
    ] {
        let blocker = fixture.vertex(point.x, point.y);
        fixture.edge(end, blocker);
    }
    fixture.finish(
        "all_candidates_collapse",
        GeometricConstraintKindV1::FixedLength {
            edge: target,
            length_mm: 3.0,
        },
        1,
    )
}

fn subnormal_fixed_angle_case() -> SingletonCase {
    let mut fixture = FixtureBuilder::default();
    let center = fixture.vertex(0.0, 0.0);
    let first_end = fixture.vertex(1.0, 0.0);
    let second_end = fixture.vertex(0.0, 1.0);
    let first = fixture.edge(center, first_end);
    let second = fixture.edge(center, second_end);
    fixture.finish(
        "subnormal_fixed_angle",
        GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: first,
            second_edge: second,
            angle_degrees: f64::from_bits(1),
        },
        1,
    )
}

#[test]
fn unsupported_or_collapsing_singleton_construction_falls_through_without_a_false_positive() {
    for case in [
        four_translation_collapse_case(),
        subnormal_fixed_angle_case(),
    ] {
        assert!(
            ori_core::construct_single_constraint_exact_assignment_v1(
                &case.pattern,
                &case.document,
            )
            .is_none(),
            "{} must remain outside the constructor",
            case.name,
        );
        assert!(
            !matches!(
                analyze_geometric_constraint_document(&case.pattern, &case.document),
                GeometricConstraintPreflightResult::ProvenSatisfiable { .. }
            ),
            "{} must fall through to the previous fail-closed outcome",
            case.name,
        );
    }
}

#[test]
fn bounded_multi_record_constructive_sat_respects_the_existing_geometry_resource_envelope() {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(0.0, 0.0);
    let end = fixture.vertex(3.0, 4.0);
    let edge = fixture.edge(start, end);
    while fixture.vertices.len() <= ori_core::DEFAULT_MAX_CONSTRAINT_VERTICES {
        let ordinal = fixture.vertices.len() as f64;
        fixture.vertex(10_000.0 + ordinal, 20_000.0 + ordinal);
    }
    let pattern = CreasePattern {
        vertices: fixture.vertices,
        edges: fixture.edges,
    };
    let mut constraints = vec![GeometricConstraintKindV1::FixedLength {
        edge,
        length_mm: 1.0,
    }];
    constraints.extend(
        (1..ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1)
            .map(|_| GeometricConstraintKindV1::Horizontal { edge }),
    );
    let document = document(constraints);

    assert!(
        ori_core::construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &document)
            .is_none(),
    );
    assert!(matches!(
        analyze_geometric_constraint_document(&pattern, &document),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len()
            == ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
    ));
}

fn assert_constructed_sat_publication_rechecks_cancel_and_deadline(
    document: &GeometricConstraintDocumentV1,
    certificate: ori_core::Binary64ExactConstraintSatisfactionV1,
) {
    let mut unchecked_constraint_ids = document
        .constraints
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    unchecked_constraint_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    for (runtime, expected_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future cancellation deadline"),
            },
            GeometricConstraintUnknownReason::Cancelled,
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            GeometricConstraintUnknownReason::DeadlineReached,
        ),
    ] {
        assert_eq!(
            finish_constructed_exact_geometric_constraint_satisfaction(
                document,
                &mut GeometricConstraintAnalysisObserver::new(runtime),
                certificate,
            ),
            GeometricConstraintPreflightResult::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: unchecked_constraint_ids.clone(),
            },
        );
    }
}

#[test]
fn constructive_attempt_post_checkpoint_covers_some_and_none_results() {
    let positive = one_edge_case("horizontal", |edge| GeometricConstraintKindV1::Horizontal {
        edge,
    });
    let assignment = ori_core::construct_single_constraint_exact_assignment_v1(
        &positive.pattern,
        &positive.document,
    )
    .expect("positive control must construct");
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut cancelled_observer =
        GeometricConstraintAnalysisObserver::new(GeometricConstraintAnalysisRuntime {
            cancellation: Arc::clone(&cancellation),
            deadline: std::time::Instant::now()
                .checked_add(Duration::from_secs(60))
                .expect("future cancellation deadline"),
        });
    cancellation.store(true, Ordering::Release);
    assert!(matches!(
        recheck_after_constructive_assignment_attempt(&mut cancelled_observer, Some(assignment),),
        Err(GeometricConstraintAnalysisStop::Cancelled),
    ));

    let unsupported = four_translation_collapse_case();
    let no_assignment = ori_core::construct_single_constraint_exact_assignment_v1(
        &unsupported.pattern,
        &unsupported.document,
    );
    assert!(no_assignment.is_none());
    let mut expired_observer =
        GeometricConstraintAnalysisObserver::new(GeometricConstraintAnalysisRuntime {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: std::time::Instant::now(),
        });
    assert!(matches!(
        recheck_after_constructive_assignment_attempt(&mut expired_observer, no_assignment),
        Err(GeometricConstraintAnalysisStop::DeadlineReached),
    ));
}

#[test]
fn constructed_sat_publication_rechecks_cancel_and_deadline() {
    let case = one_edge_case("horizontal", |edge| GeometricConstraintKindV1::Horizontal {
        edge,
    });
    let assignment =
        ori_core::construct_single_constraint_exact_assignment_v1(&case.pattern, &case.document)
            .expect("the singleton control must construct");
    assert_constructed_sat_publication_rechecks_cancel_and_deadline(
        &case.document,
        assignment.certificate(),
    );
}

#[test]
fn constructed_two_record_sat_publication_rechecks_cancel_and_deadline() {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(0.0, 0.0);
    let end = fixture.vertex(3.0, 4.0);
    let edge = fixture.edge(start, end);
    let pattern = CreasePattern {
        vertices: fixture.vertices,
        edges: fixture.edges,
    };
    let document = document([
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        },
        GeometricConstraintKindV1::Horizontal { edge },
    ]);
    let assignment = ori_core::construct_two_constraint_exact_assignment_v1(&pattern, &document)
        .expect("the compatible two-record control must construct");
    assert_constructed_sat_publication_rechecks_cancel_and_deadline(
        &document,
        assignment.certificate(),
    );
}

#[test]
fn constructed_three_and_sixteen_record_sat_publication_rechecks_cancel_and_deadline() {
    let mut fixture = FixtureBuilder::default();
    let start = fixture.vertex(0.0, 0.0);
    let end = fixture.vertex(3.0, 4.0);
    let edge = fixture.edge(start, end);
    let pattern = CreasePattern {
        vertices: fixture.vertices,
        edges: fixture.edges,
    };
    let mut document = document([
        GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        },
        GeometricConstraintKindV1::Horizontal { edge },
        GeometricConstraintKindV1::Horizontal { edge },
    ]);
    let assignment = ori_core::construct_three_constraint_exact_assignment_v1(&pattern, &document)
        .expect("the compatible three-record control must construct");
    assert_constructed_sat_publication_rechecks_cancel_and_deadline(
        &document,
        assignment.certificate(),
    );

    while document.constraints.len() < ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
        document
            .constraints
            .push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    let assignment =
        ori_core::construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &document)
            .expect("the compatible sixteen-record control must construct");
    assert_constructed_sat_publication_rechecks_cancel_and_deadline(
        &document,
        assignment.certificate(),
    );
}

#[derive(Debug, PartialEq)]
struct ProjectSignature {
    instance_id: ProjectId,
    project_id: ProjectId,
    document: ProjectDocument,
    editor_debug: String,
    revision: u64,
    can_undo: bool,
    can_redo: bool,
    dirty: bool,
    current_path: Option<PathBuf>,
    saved_revision: Option<u64>,
    saved_document: Option<ProjectDocument>,
}

fn project_signature(state: &AppState) -> ProjectSignature {
    let project = lock_project(state).expect("lock singleton SAT project");
    ProjectSignature {
        instance_id: project.instance_id,
        project_id: project.project_id,
        document: project.document(),
        editor_debug: format!("{:?}", project.editor),
        revision: project.editor.revision(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        dirty: project.is_dirty(),
        current_path: project.current_path.clone(),
        saved_revision: project.saved_revision,
        saved_document: project.saved_document.clone(),
    }
}

#[test]
fn worker_publishes_singleton_sat_without_changing_document_revision_history_or_dirty_state() {
    let mut project = initial_project_state();
    let edge = {
        let pattern = project.editor.pattern();
        pattern
            .edges
            .iter()
            .find(|edge| {
                let start = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)
                    .expect("edge start");
                let end = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.end)
                    .expect("edge end");
                start.position.y != end.position.y
            })
            .expect("the startup sheet has a non-horizontal edge")
            .id
    };
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    execute_command(
        &mut project,
        project_instance_id,
        project_id,
        revision,
        Command::AddGeometricConstraint {
            record: record(GeometricConstraintKindV1::Horizontal { edge }),
        },
    )
    .expect("add the unsatisfied singleton constraint");
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            project.editor.pattern(),
            project.editor.geometric_constraints(),
        )
        .expect("the project constraint document is valid")
        .is_none(),
        "the worker fixture must require constructive SAT",
    );

    let state = AppState::new(project);
    let before = project_signature(&state);
    let binding = (before.instance_id, before.project_id, before.revision);
    let response = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        |pattern, document, _runtime| {
            Ok(analyze_geometric_constraint_document(&pattern, &document))
        },
    ))
    .expect("run singleton SAT worker");

    assert_eq!(response.result, expected_positive(1));
    assert!(response.semantic_mus.is_none());
    assert_eq!(project_signature(&state), before);
}

#[test]
fn worker_publishes_bounded_multi_record_sat_without_changing_project_state() {
    for horizontal_count in [1_usize, 2, 3, 7, 15] {
        let mut project = initial_project_state();
        let edge = {
            let pattern = project.editor.pattern();
            pattern
                .edges
                .iter()
                .find(|edge| {
                    let start = pattern
                        .vertices
                        .iter()
                        .find(|vertex| vertex.id == edge.start)
                        .expect("edge start");
                    let end = pattern
                        .vertices
                        .iter()
                        .find(|vertex| vertex.id == edge.end)
                        .expect("edge end");
                    start.position.y != end.position.y
                })
                .expect("the startup sheet has a non-horizontal edge")
                .id
        };
        let project_instance_id = project.instance_id;
        let project_id = project.project_id;
        let mut constraints = vec![GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }];
        constraints
            .extend((0..horizontal_count).map(|_| GeometricConstraintKindV1::Horizontal { edge }));
        for constraint in constraints {
            let revision = project.editor.revision();
            execute_command(
                &mut project,
                project_instance_id,
                project_id,
                revision,
                Command::AddGeometricConstraint {
                    record: record(constraint),
                },
            )
            .expect("add one compatible bounded-composition constraint");
        }
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                project.editor.pattern(),
                project.editor.geometric_constraints(),
            )
            .expect("the project constraint document is valid")
            .is_none(),
            "the worker fixture must require composed constructive SAT",
        );

        let state = AppState::new(project);
        let before = project_signature(&state);
        let binding = (before.instance_id, before.project_id, before.revision);
        let response = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &state,
            binding.0,
            binding.1,
            binding.2,
            ProjectId::new(),
            |pattern, document, _runtime| {
                Ok(analyze_geometric_constraint_document(&pattern, &document))
            },
        ))
        .expect("run bounded composed SAT worker");

        let constraint_count = horizontal_count + 1;
        assert_eq!(
            response.result,
            expected_positive_document(constraint_count, constraint_count),
        );
        assert!(response.semantic_mus.is_none());
        assert_eq!(project_signature(&state), before);
    }
}
