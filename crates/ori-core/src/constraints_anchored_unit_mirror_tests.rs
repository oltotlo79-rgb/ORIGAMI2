use std::collections::HashMap;

use ori_domain::{EdgeKind, Point2};

use super::*;

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 4],
    edges: [EdgeId; 3],
}

impl Fixture {
    fn new() -> Self {
        let mut vertices = std::array::from_fn(|_| VertexId::new());
        if vertices[2].canonical_bytes() > vertices[3].canonical_bytes() {
            vertices.swap(2, 3);
        }
        let edges = std::array::from_fn(|_| EdgeId::new());
        Self {
            pattern: CreasePattern {
                vertices: vec![
                    Vertex {
                        id: vertices[0],
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: vertices[1],
                        position: Point2::new(1.0, 0.0),
                    },
                    Vertex {
                        id: vertices[2],
                        position: Point2::new(0.0, 0.5),
                    },
                    Vertex {
                        id: vertices[3],
                        position: Point2::new(0.0, -0.5),
                    },
                ],
                edges: vec![
                    Edge {
                        id: edges[0],
                        start: vertices[0],
                        end: vertices[1],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[1],
                        start: vertices[0],
                        end: vertices[2],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[2],
                        start: vertices[2],
                        end: vertices[3],
                        kind: EdgeKind::Auxiliary,
                    },
                ],
            },
            vertices,
            edges,
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
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

fn core_records(fixture: &Fixture, separation_length: f64) -> [GeometricConstraintRecordV1; 7] {
    [
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[2],
            second_vertex: fixture.vertices[3],
            axis_edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[2],
            length_mm: separation_length,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[1],
        }),
    ]
}

fn vertical_axis_records(
    fixture: &Fixture,
    separation_length: f64,
) -> [GeometricConstraintRecordV1; 7] {
    let mut records = core_records(fixture, separation_length);
    records[4].constraint = GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[0],
    };
    records
}

fn minimal_anchored_records(
    fixture: &Fixture,
    separation_length: f64,
) -> [GeometricConstraintRecordV1; 4] {
    let records = core_records(fixture, separation_length);
    [
        records[0].clone(),
        records[2].clone(),
        records[5].clone(),
        records[6].clone(),
    ]
}

fn anchored_proof_ids(records: &[GeometricConstraintRecordV1; 7]) -> Vec<ConstraintId> {
    sorted_ids(&[
        records[0].clone(),
        records[2].clone(),
        records[5].clone(),
        records[6].clone(),
    ])
}

fn prepare<'a>(
    fixture: &'a Fixture,
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(constraints),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("anchored unit-mirror fixture prepares")
}

fn sorted_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn assert_production_residuals_are_exact_zero(
    fixture: &Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
    positions: [Point2; 4],
) {
    let positions = fixture
        .vertices
        .iter()
        .copied()
        .zip(positions)
        .collect::<HashMap<_, _>>();
    let values = crate::constraint_solver::deterministic_proof_residuals_v1(
        &fixture.pattern,
        &document(records),
        &positions,
    )
    .expect("the explicit production-residual assignment evaluates");
    assert!(
        values.iter().all(|value| *value == 0.0),
        "every retained production residual must be exact zero: {values:?}"
    );
}

fn canonical_mirrored_vertices(fixture: &Fixture) -> (VertexId, VertexId) {
    if fixture.vertices[2].canonical_bytes() < fixture.vertices[3].canonical_bytes() {
        (fixture.vertices[2], fixture.vertices[3])
    } else {
        (fixture.vertices[3], fixture.vertices[2])
    }
}

#[test]
fn four_record_anchored_mirror_core_has_an_exact_residual_witness_after_each_deletion() {
    let fixture = Fixture::new();
    let records = minimal_anchored_records(&fixture, 2.0);
    let expected_ids = sorted_ids(&records);
    let prepared = prepare(&fixture, records.clone());
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
        panic!("the four-record raw-source anchor must be a direct contradiction");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);

    let assignments = [
        // Without MirrorSymmetry, the anchored source and a target two units
        // away satisfy the fixed separation and both connector residuals.
        [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
        ],
        // Without FixedLength, both symmetry points may coincide at the axis
        // start. This is a residual witness rather than a persistence fixture:
        // validation intentionally rejects coincident mirror roles.
        [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
        ],
        // Without Horizontal on the connector, Vertical permits a source
        // above a horizontal axis and its exact reflected target below it.
        [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(0.0, -1.0),
        ],
        // Without Vertical on the connector, Horizontal permits a source to
        // the right of a vertical axis and its exact target to the left.
        [
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(1.0, 0.0),
            Point2::new(-1.0, 0.0),
        ],
    ];
    for (omitted, positions) in assignments.into_iter().enumerate() {
        assert_production_residuals_are_exact_zero(
            &fixture,
            records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, record)| record.clone()),
            positions,
        );
    }
}

#[test]
fn anchored_unit_mirror_emits_canonical_order_independent_direct_unsat_evidence() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 1.0);
    let expected_ids = anchored_proof_ids(&records);
    let (first_vertex, second_vertex) = canonical_mirrored_vertices(&fixture);
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict:
                DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                    first_vertex,
                    second_vertex,
                    axis_edge: fixture.edges[0],
                    fixed_separation_edge: fixture.edges[2],
                },
            constraint_ids: expected_ids.clone(),
        }],
    };
    let prepared = prepare(&fixture, records.clone());
    assert_eq!(prepared.preflight(), expected);
    let mut reversed = records.to_vec();
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, reversed).preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                            first_vertex,
                            second_vertex,
                            axis_edge: fixture.edges[0],
                            fixed_separation_edge: fixture.edges[2],
                        },
                constraint_ids: expected_ids.clone(),
            }],
        }
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
        find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the anchored exact mirror slice must feed the bounded UNSAT oracle");
    };
    assert_eq!(constraint_ids, expected_ids);
    for removed in expected_ids.iter().copied() {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned();
        assert!(
            !matches!(
                prepare(&fixture, subset).preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "removing every listed evidence record must remove the direct contradiction"
        );
    }
}

#[test]
fn anchored_mirror_selects_the_raw_source_qualified_duplicate_across_input_and_edge_order() {
    let fixture = Fixture::new();
    let mut records = minimal_anchored_records(&fixture, 2.0).to_vec();
    let mut duplicate_mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[2],
        second_vertex: fixture.vertices[3],
        axis_edge: fixture.edges[0],
    });
    let expected_mirror = if records[0].id.canonical_bytes() < duplicate_mirror.id.canonical_bytes()
    {
        records[0].constraint = GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[3],
            second_vertex: fixture.vertices[2],
            axis_edge: fixture.edges[0],
        };
        duplicate_mirror.id
    } else {
        duplicate_mirror.constraint = GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[3],
            second_vertex: fixture.vertices[2],
            axis_edge: fixture.edges[0],
        };
        records[0].id
    };
    let expected_ids = sorted_ids(&[
        GeometricConstraintRecordV1 {
            id: expected_mirror,
            constraint: records[0].constraint.clone(),
        },
        records[1].clone(),
        records[2].clone(),
        records[3].clone(),
    ]);
    records.push(duplicate_mirror);

    let baseline = prepare(&fixture, records.clone()).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = &baseline else {
        panic!("duplicate mirrors retain the anchored direct proof");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);

    records.reverse();
    let mut reordered_pattern = fixture.pattern.clone();
    reordered_pattern.edges.reverse();
    let reordered = prepare_geometric_constraints_v1(
        &reordered_pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("reordered duplicate witness prepares")
    .preflight();
    assert_eq!(reordered, baseline);
}

#[test]
fn raw_mirror_operand_reversal_counterexample_stays_unknown_and_has_exact_zero_residuals() {
    let fixture = Fixture::new();
    let mut records = core_records(&fixture, 1.0);
    records[0].constraint = GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[3],
        second_vertex: fixture.vertices[2],
        axis_edge: fixture.edges[0],
    };
    let raw = document(records.clone());
    let prepared = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &raw,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("raw-role counterexample prepares");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::Unknown { .. }
    ));

    let huge: f64 = 9_007_199_254_740_992.0;
    let raw_source_y = huge - 1.0;
    assert_eq!(
        (2.0 * huge - raw_source_y).to_bits(),
        huge.to_bits(),
        "the reflected y coordinate is the binary64 tie-to-even counterexample"
    );
    assert_production_residuals_are_exact_zero(
        &fixture,
        records,
        [
            Point2::new(0.0, huge),
            Point2::new(1.0, huge),
            Point2::new(0.0, huge),
            Point2::new(0.0, raw_source_y),
        ],
    );
}

#[test]
fn anchored_unit_vertical_mirror_emits_the_same_exact_unsat_family() {
    let fixture = Fixture::new();
    let records = vertical_axis_records(&fixture, 1.0);
    let expected_ids = anchored_proof_ids(&records);
    let prepared = prepare(&fixture, records.clone());
    let result = prepared.preflight();
    let mut reversed = records.to_vec();
    reversed.reverse();
    assert_eq!(prepare(&fixture, reversed).preflight(), result);
    let ConstraintPreflightV1::DirectConflict { conflicts } = result else {
        panic!("an exact-unit vertical axis must use the cardinal mirror theorem");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
            axis_edge,
            fixed_separation_edge,
            ..
        } if *axis_edge == fixture.edges[0] && *fixed_separation_edge == fixture.edges[2]
    ));

    for removed in expected_ids.iter().copied() {
        assert!(
            !matches!(
                prepare(
                    &fixture,
                    records
                        .iter()
                        .filter(|record| record.id != removed)
                        .cloned(),
                )
                .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "every record in the four-record anchored witness remains necessary"
        );
    }
}

#[test]
fn anchored_mirror_without_the_complete_connector_keeps_an_exact_satisfiable_witness() {
    let fixture = Fixture::new();
    let mut satisfiable = core_records(&fixture, 1.0).into_iter().collect::<Vec<_>>();
    satisfiable.remove(1);
    let _ = satisfiable.pop();
    let _ = satisfiable.pop();
    let satisfiable_document = document(satisfiable.clone());
    assert!(
        crate::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &fixture.pattern,
            &satisfiable_document,
        )
        .expect("compatible anchored mirror model is valid")
        .is_some(),
        "the compatible counterpart has an exact binary64 SAT witness"
    );
    assert!(!matches!(
        prepare(&fixture, satisfiable).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

#[test]
fn anchored_mirror_accepts_every_tested_positive_fixed_length_without_axis_cardinality() {
    let fixture = Fixture::new();
    let nonunit_values = [2.0, 1.0_f64.next_down(), 1.0_f64.next_up()];

    for vertical_axis in [false, true] {
        for nonunit_separation in nonunit_values {
            let records = if vertical_axis {
                vertical_axis_records(&fixture, nonunit_separation)
            } else {
                core_records(&fixture, nonunit_separation)
            };
            let expected_ids = anchored_proof_ids(&records);
            let ConstraintPreflightV1::DirectConflict { conflicts } =
                prepare(&fixture, records).preflight()
            else {
                panic!("every positive separation remains contradictory");
            };
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].constraint_ids(), expected_ids);
        }

        for nonunit_axis in nonunit_values {
            let mut records = if vertical_axis {
                vertical_axis_records(&fixture, 1.0)
            } else {
                core_records(&fixture, 1.0)
            };
            records[3].constraint = GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: nonunit_axis,
            };
            let expected_ids = anchored_proof_ids(&records);
            let ConstraintPreflightV1::DirectConflict { conflicts } =
                prepare(&fixture, records).preflight()
            else {
                panic!("axis fixed length and orientation are not proof causes");
            };
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].constraint_ids(), expected_ids);
        }
    }
}

#[test]
fn anchored_unit_mirror_preserves_resource_cancel_and_deadline_unknown_boundaries() {
    let fixture = Fixture::new();
    let prepared = prepare(&fixture, core_records(&fixture, 1.0));
    let expected_ids = sorted_ids(prepared.constraints());
    let limited = GeometricConstraintSetV1 {
        source_pattern: &fixture.pattern,
        constraints: prepared.constraints.clone(),
        raw_mirror_roles: prepared.raw_mirror_roles.clone(),
        max_preflight_checks: 6,
    };
    assert!(matches!(
        limited.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
    ));

    struct StopOnSecondCheckpoint {
        calls: usize,
        control: GeometricConstraintPreflightObserverControlV1,
    }
    impl GeometricConstraintPreflightObserverV1 for StopOnSecondCheckpoint {
        fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
            self.calls += 1;
            if self.calls == 2 {
                self.control
            } else {
                GeometricConstraintPreflightObserverControlV1::Continue
            }
        }
    }
    for control in [
        GeometricConstraintPreflightObserverControlV1::Cancelled,
        GeometricConstraintPreflightObserverControlV1::DeadlineReached,
    ] {
        let expected_reason = match control {
            GeometricConstraintPreflightObserverControlV1::Cancelled => {
                GeometricConstraintUnknownReasonV1::Cancelled
            }
            GeometricConstraintPreflightObserverControlV1::DeadlineReached => {
                GeometricConstraintUnknownReasonV1::DeadlineReached
            }
            GeometricConstraintPreflightObserverControlV1::Continue => unreachable!(),
        };
        let mut observer = StopOnSecondCheckpoint { calls: 0, control };
        assert_eq!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: expected_ids.clone(),
            }
        );
        assert_eq!(
            observer.calls, 2,
            "the stop must occur after the direct candidate was constructed"
        );
    }
}
