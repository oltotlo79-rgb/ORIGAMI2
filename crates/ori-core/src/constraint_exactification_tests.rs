use std::collections::BTreeMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use crate::{
    ConstraintSolveLimitsV1, ConstraintSolvePreviewV1,
    GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    exactify_axis_aligned_constraint_preview_v1, solve_geometric_constraints_v1,
};

fn vertex(id: VertexId, x: f64, y: f64) -> Vertex {
    Vertex {
        id,
        position: Point2::new(x, y),
    }
}

fn edge(id: EdgeId, start: VertexId, end: VertexId) -> Edge {
    Edge {
        id,
        start,
        end,
        kind: EdgeKind::Auxiliary,
    }
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

fn preview(positions: Vec<(VertexId, Point2)>) -> ConstraintSolvePreviewV1 {
    ConstraintSolvePreviewV1 {
        positions,
        iterations: 0,
        maximum_residual: 0.0,
        rank: 0,
        degrees_of_freedom: 0,
        equation_count: 0,
        condition_estimate: 1.0,
    }
}

fn apply_preview(
    pattern: &CreasePattern,
    solve_preview: &ConstraintSolvePreviewV1,
) -> CreasePattern {
    let mut candidate = pattern.clone();
    for (id, point) in &solve_preview.positions {
        candidate
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == *id)
            .expect("solver preview vertex")
            .position = *point;
    }
    candidate
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

fn next_up(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn numerical_horizontal_preview_is_promoted_only_after_exact_projection() {
    let start = VertexId::new();
    let end = VertexId::new();
    let untouched = VertexId::new();
    let target = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            vertex(start, 0.0, 0.0),
            vertex(end, 4.0, 1.0),
            vertex(untouched, 9.0, 9.0),
        ],
        edges: vec![edge(target, start, end)],
    };
    let constraints = document([record(GeometricConstraintKindV1::Horizontal {
        edge: target,
    })]);
    let numerical = solve_geometric_constraints_v1(
        &pattern,
        &constraints,
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("bounded numerical solve");
    let numerical_pattern = apply_preview(&pattern, &numerical);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &numerical_pattern,
            &constraints,
        )
        .expect("numerical candidate remains structurally valid")
        .is_none(),
        "tolerance convergence alone must not issue an exact witness",
    );

    let pattern_before = pattern.clone();
    let constraints_before = constraints.clone();
    let numerical_before = numerical.clone();
    let exact = exactify_axis_aligned_constraint_preview_v1(&pattern, &constraints, &numerical)
        .expect("axis projection should produce an exact assignment");
    assert_eq!(pattern, pattern_before);
    assert_eq!(constraints, constraints_before);
    assert_eq!(numerical, numerical_before);
    assert_eq!(
        exact.model_id(),
        GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
    );
    assert!(!exact.authorizes_project_mutation());
    assert!(!exact.replayable_across_runtimes());
    assert_eq!(exact.certificate().constraint_count(), 1);
    assert_eq!(exact.certificate().equation_count(), 1);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(exact.pattern(), &constraints,)
            .expect("explicit assignment remains valid")
            .is_some(),
    );
    assert_eq!(
        exact
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == untouched)
            .expect("unspecified vertex")
            .position,
        Point2::new(9.0, 9.0),
        "a vertex omitted from the preview and every axis class keeps its original position",
    );
}

#[test]
fn projection_rechecks_every_non_axis_residual() {
    let first_start = VertexId::new();
    let first_end = VertexId::new();
    let second_start = VertexId::new();
    let second_end = VertexId::new();
    let first = EdgeId::new();
    let second = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            vertex(first_start, 0.0, 0.0),
            vertex(first_end, 2.0, 1.0),
            vertex(second_start, 0.0, 3.0),
            vertex(second_end, 2.0, 3.0),
        ],
        edges: vec![
            edge(first, first_start, first_end),
            edge(second, second_start, second_end),
        ],
    };
    let satisfiable = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: first,
            second_edge: second,
        }),
    ]);
    assert!(
        exactify_axis_aligned_constraint_preview_v1(&pattern, &satisfiable, &preview(Vec::new()),)
            .is_some(),
        "equal length must be re-certified after the exact axis projection",
    );

    let nonzero = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 3.0,
        }),
    ]);
    assert!(
        exactify_axis_aligned_constraint_preview_v1(&pattern, &nonzero, &preview(Vec::new()),)
            .is_none(),
        "a forged zero diagnostic cannot hide the nonzero fixed-length residual",
    );
}

#[test]
fn canonical_axis_classes_are_invariant_to_all_storage_orders() {
    let first = VertexId::new();
    let second = VertexId::new();
    let third = VertexId::new();
    let fourth = VertexId::new();
    let fifth = VertexId::new();
    let horizontal_one = EdgeId::new();
    let horizontal_two = EdgeId::new();
    let vertical = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            vertex(first, 0.0, 7.0),
            vertex(second, 1.0, 8.0),
            vertex(third, 2.0, 9.0),
            vertex(fourth, 4.0, 0.0),
            vertex(fifth, 5.0, 3.0),
        ],
        edges: vec![
            edge(horizontal_one, first, second),
            edge(horizontal_two, second, third),
            edge(vertical, fourth, fifth),
        ],
    };
    let constraints = document([
        record(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_one,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_two,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: vertical }),
    ]);
    let forward_preview = preview(vec![
        (third, Point2::new(2.0, 11.0)),
        (fifth, Point2::new(6.0, 3.0)),
    ]);
    let pattern_before = pattern.clone();
    let constraints_before = constraints.clone();
    let preview_before = forward_preview.clone();
    let expected =
        exactify_axis_aligned_constraint_preview_v1(&pattern, &constraints, &forward_preview)
            .expect("ordered axis system");
    assert_eq!(pattern, pattern_before);
    assert_eq!(constraints, constraints_before);
    assert_eq!(forward_preview, preview_before);

    let mut reversed_pattern = pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    let mut reversed_constraints = constraints.clone();
    reversed_constraints.constraints.reverse();
    let reversed_preview = preview(forward_preview.positions.iter().copied().rev().collect());
    let actual = exactify_axis_aligned_constraint_preview_v1(
        &reversed_pattern,
        &reversed_constraints,
        &reversed_preview,
    )
    .expect("reordered axis system");

    assert_eq!(
        position_bits(actual.pattern()),
        position_bits(expected.pattern()),
    );
}

#[test]
fn invalid_preview_and_projection_collapse_fail_closed() {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![vertex(start, 0.0, 0.0), vertex(end, 1.0, 1.0)],
        edges: vec![edge(target, start, end)],
    };
    let horizontal = document([record(GeometricConstraintKindV1::Horizontal {
        edge: target,
    })]);
    for invalid in [
        preview(vec![
            (start, Point2::new(0.0, 0.0)),
            (start, Point2::new(1.0, 0.0)),
        ]),
        preview(vec![(VertexId::new(), Point2::new(0.0, 0.0))]),
        preview(vec![(start, Point2::new(f64::NAN, 0.0))]),
        preview(vec![
            (start, Point2::new(0.0, 0.0)),
            (end, Point2::new(1.0, 1.0)),
            (VertexId::new(), Point2::new(2.0, 2.0)),
        ]),
    ] {
        assert!(
            exactify_axis_aligned_constraint_preview_v1(&pattern, &horizontal, &invalid).is_none(),
        );
    }

    let collapsed = document([
        record(GeometricConstraintKindV1::Horizontal { edge: target }),
        record(GeometricConstraintKindV1::Vertical { edge: target }),
    ]);
    assert!(
        exactify_axis_aligned_constraint_preview_v1(&pattern, &collapsed, &preview(Vec::new()),)
            .is_none(),
        "projection may not turn a required geometry edge into a degenerate edge",
    );
}

#[test]
fn direct_conflict_and_one_ulp_non_axis_residual_fail_closed() {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![vertex(start, 0.0, 0.0), vertex(end, next_up(2.0), 0.0)],
        edges: vec![edge(target, start, end)],
    };
    let one_ulp = document([record(GeometricConstraintKindV1::FixedLength {
        edge: target,
        length_mm: 2.0,
    })]);
    assert!(
        exactify_axis_aligned_constraint_preview_v1(&pattern, &one_ulp, &preview(Vec::new()),)
            .is_none(),
        "a one-ULP non-axis residual is not numerical tolerance",
    );

    let direct_conflict = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: target,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: target,
            length_mm: 3.0,
        }),
    ]);
    assert!(
        exactify_axis_aligned_constraint_preview_v1(
            &pattern,
            &direct_conflict,
            &preview(Vec::new()),
        )
        .is_none(),
        "a proven direct conflict cannot be overridden by candidate projection",
    );
}

#[test]
fn preview_diagnostics_are_non_authoritative_and_do_not_change_the_certificate() {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![vertex(start, 0.0, 0.0), vertex(end, 2.0, 1.0)],
        edges: vec![edge(target, start, end)],
    };
    let constraints = document([record(GeometricConstraintKindV1::Horizontal {
        edge: target,
    })]);
    let ordinary = preview(vec![(end, Point2::new(2.0, 3.0))]);
    let forged = ConstraintSolvePreviewV1 {
        positions: ordinary.positions.clone(),
        iterations: usize::MAX,
        maximum_residual: f64::NAN,
        rank: usize::MAX,
        degrees_of_freedom: usize::MAX,
        equation_count: usize::MAX,
        condition_estimate: f64::INFINITY,
    };

    let expected = exactify_axis_aligned_constraint_preview_v1(&pattern, &constraints, &ordinary)
        .expect("ordinary diagnostics");
    let actual = exactify_axis_aligned_constraint_preview_v1(&pattern, &constraints, &forged)
        .expect("forged diagnostics cannot override independent recertification");

    assert_eq!(
        position_bits(actual.pattern()),
        position_bits(expected.pattern()),
    );
    assert_eq!(actual.certificate(), expected.certificate());
}

#[test]
fn empty_document_is_not_an_exact_assignment_claim() {
    let lone = VertexId::new();
    let pattern = CreasePattern {
        vertices: vec![vertex(lone, 1.0, 2.0)],
        edges: Vec::new(),
    };

    assert!(
        exactify_axis_aligned_constraint_preview_v1(&pattern, &document([]), &preview(Vec::new()),)
            .is_none(),
        "an empty document must not produce a vacuous witness",
    );
}

#[test]
fn already_exact_non_axis_assignment_is_recertified_without_projection() {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![vertex(start, 0.0, 0.0), vertex(end, 2.0, 0.0)],
        edges: vec![edge(target, start, end)],
    };
    let constraints = document([record(GeometricConstraintKindV1::FixedLength {
        edge: target,
        length_mm: 2.0,
    })]);

    let exact =
        exactify_axis_aligned_constraint_preview_v1(&pattern, &constraints, &preview(Vec::new()))
            .expect("an existing exact non-axis assignment remains certifiable");
    assert_eq!(position_bits(exact.pattern()), position_bits(&pattern));
    assert_eq!(exact.certificate().constraint_count(), 1);
    assert_eq!(exact.certificate().equation_count(), 1);
}

#[test]
fn invalid_source_pattern_fails_before_preview_projection() {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = EdgeId::new();
    let base = CreasePattern {
        vertices: vec![vertex(start, 0.0, 0.0), vertex(end, 1.0, 1.0)],
        edges: vec![edge(target, start, end)],
    };
    let constraints = document([record(GeometricConstraintKindV1::Horizontal {
        edge: target,
    })]);

    let mut duplicate = base.clone();
    duplicate.vertices.push(vertex(start, 4.0, 5.0));
    assert!(
        exactify_axis_aligned_constraint_preview_v1(
            &duplicate,
            &constraints,
            &preview(Vec::new()),
        )
        .is_none(),
        "duplicate source vertex IDs are rejected",
    );

    let mut nonfinite = base.clone();
    nonfinite.vertices[0].position.x = f64::NAN;
    assert!(
        exactify_axis_aligned_constraint_preview_v1(
            &nonfinite,
            &constraints,
            &preview(Vec::new()),
        )
        .is_none(),
        "nonfinite source geometry is rejected",
    );
}
