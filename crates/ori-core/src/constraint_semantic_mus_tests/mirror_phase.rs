use super::*;
use crate::ConstraintPreflightV1;

pub(super) struct AnchoredMirrorFixture {
    pub(super) fixture: SemanticFixture,
    pub(super) axis_start: VertexId,
    pub(super) axis_end: VertexId,
    pub(super) raw_source: VertexId,
    pub(super) raw_target: VertexId,
    pub(super) axis_edge: EdgeId,
}

pub(super) fn anchored_mirror_fixture(
    separation_length: f64,
    raw_source_is_canonical_high: bool,
) -> AnchoredMirrorFixture {
    let axis_start = VertexId::new();
    let axis_end = VertexId::new();
    let mut symmetry_vertices = [VertexId::new(), VertexId::new()];
    symmetry_vertices.sort_unstable_by_key(VertexId::canonical_bytes);
    let (raw_source, raw_target) = if raw_source_is_canonical_high {
        (symmetry_vertices[1], symmetry_vertices[0])
    } else {
        (symmetry_vertices[0], symmetry_vertices[1])
    };
    let axis_edge = EdgeId::new();
    let connector_edge = EdgeId::new();
    let separation_edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: axis_start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: axis_end,
                position: Point2::new(2.0, 1.0),
            },
            Vertex {
                id: raw_source,
                position: Point2::new(3.0, 5.0),
            },
            Vertex {
                id: raw_target,
                position: Point2::new(7.0, 11.0),
            },
        ],
        edges: vec![
            Edge {
                id: axis_edge,
                start: axis_start,
                end: axis_end,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: connector_edge,
                start: axis_start,
                end: raw_source,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: separation_edge,
                start: raw_source,
                end: raw_target,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: raw_source,
            second_vertex: raw_target,
            axis_edge,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: separation_edge,
            length_mm: separation_length,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: connector_edge,
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: connector_edge,
        }),
    ];
    AnchoredMirrorFixture {
        fixture: SemanticFixture { pattern, records },
        axis_start,
        axis_end,
        raw_source,
        raw_target,
        axis_edge,
    }
}

pub(super) fn anchored_mirror_inventory_fixture() -> SemanticFixture {
    anchored_mirror_fixture(2.0, true).fixture
}

fn certificate_for(fixture: &SemanticFixture) -> CurrentRuntimeSemanticMusV1 {
    certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared(
        &fixture.pattern,
        fixture.records.iter().cloned(),
    )))
}

fn assert_only_mirror_phase(certificate: &CurrentRuntimeSemanticMusV1) {
    assert_eq!(certificate.current_assignment_witness_count(), 0);
    assert_eq!(certificate.axis_exactification_witness_count(), 0);
    assert_eq!(
        certificate.single_constraint_constructive_witness_count(),
        0
    );
    assert_eq!(certificate.pair_constraint_constructive_witness_count(), 0);
    assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
    assert_eq!(
        certificate.length_constraint_constructive_witness_count(),
        0
    );
    assert_eq!(
        certificate.zero_length_closure_constructive_witness_count(),
        0
    );
    assert_eq!(certificate.anchored_mirror_residual_only_witness_count(), 4);
    assert_eq!(
        certificate.unit_two_hop_parallel_residual_only_witness_count(),
        0
    );
}

fn phase_work(fixture: &SemanticFixture) -> (usize, usize) {
    let (setup, ..) = crate::constraint_semantic_mus::witness_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        4,
        3,
    )
    .expect("bounded mirror setup work");
    let mirror = crate::constraint_semantic_mus::anchored_mirror_residual_only_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        3,
    )
    .expect("bounded mirror deletion work");
    (setup, mirror)
}

#[test]
fn all_four_deletions_use_finite_complete_raw_role_preserving_overlays() {
    let fixture = anchored_mirror_fixture(2.0, true);
    let expected = sorted_ids(fixture.fixture.records.iter().cloned());
    let prepared_set = prepared(
        &fixture.fixture.pattern,
        fixture.fixture.records.iter().cloned(),
    );
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared_set.preflight() else {
        panic!("the raw-source anchored four-record theorem must be direct");
    };
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            crate::DirectConstraintConflictKindV1::
                MirrorSymmetryWithPointOnAxisAndFixedSeparation { .. }
        ) && conflict.constraint_ids() == expected
    }));
    for removed in &fixture.fixture.records {
        assert!(
            !matches!(
                prepared(
                    &fixture.fixture.pattern,
                    fixture
                        .fixture
                        .records
                        .iter()
                        .filter(|record| record.id != removed.id)
                        .cloned(),
                )
                .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "every cause deletion must remove the direct theorem",
        );
    }

    let certificate = certificate_for(&fixture.fixture);
    assert_eq!(certificate.constraint_ids(), expected);
    assert_eq!(certificate.deletion_witness_checks(), 4);
    assert_only_mirror_phase(&certificate);
}

#[test]
fn normalized_clone_does_not_reverse_the_production_mirror_operands() {
    let fixture = anchored_mirror_fixture(2.0, true);
    assert!(
        fixture.raw_source.canonical_bytes() > fixture.raw_target.canonical_bytes(),
        "fixture must force persistence normalization to reverse the raw roles",
    );
    let prepared = prepared(
        &fixture.fixture.pattern,
        fixture.fixture.records.iter().cloned(),
    );
    let normalized_mirror = prepared
        .constraints()
        .iter()
        .find(|record| {
            matches!(
                record.constraint,
                GeometricConstraintKindV1::MirrorSymmetry { .. }
            )
        })
        .expect("one normalized mirror");
    assert!(matches!(
        &normalized_mirror.constraint,
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            ..
        } if *first_vertex == fixture.raw_target && *second_vertex == fixture.raw_source
    ));
    assert_only_mirror_phase(&certificate_for(&fixture.fixture));
}

#[test]
fn raw_operand_reversal_at_two_to_the_fifty_third_remains_satisfiable_and_unknown() {
    let mut fixture = anchored_mirror_fixture(1.0, false);
    fixture.fixture.records[0].constraint = GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.raw_target,
        second_vertex: fixture.raw_source,
        axis_edge: fixture.axis_edge,
    };
    let raw_document = document(fixture.fixture.records.iter().cloned());
    let prepared = prepared(
        &fixture.fixture.pattern,
        fixture.fixture.records.iter().cloned(),
    );
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown { .. }
    ));
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            ..
        }
    ));

    let huge: f64 = 9_007_199_254_740_992.0;
    let raw_source_y = huge - 1.0;
    assert_eq!((2.0 * huge - raw_source_y).to_bits(), huge.to_bits());
    let overlay = fixture
        .fixture
        .pattern
        .vertices
        .iter()
        .map(|vertex| {
            let point = if vertex.id == fixture.axis_start {
                Point2::new(0.0, huge)
            } else if vertex.id == fixture.axis_end {
                Point2::new(1.0, huge)
            } else if vertex.id == fixture.raw_source {
                Point2::new(0.0, huge)
            } else if vertex.id == fixture.raw_target {
                Point2::new(0.0, raw_source_y)
            } else {
                unreachable!()
            };
            (vertex.id, point)
        })
        .collect::<Vec<_>>();
    assert!(
        crate::constraint_solver::certify_binary64_residual_only_constraint_overlay_v1(
            &fixture.fixture.pattern,
            &raw_document,
            &overlay,
        )
        .expect("the complete finite reversal overlay is valid")
        .is_some(),
        "the 2^53 rounding counterexample forbids promotion of the reversed raw role",
    );
}

#[test]
fn work_is_reserved_exactly_and_one_short_cannot_start_the_last_overlay() {
    let fixture = anchored_mirror_fixture(2.0, true).fixture;
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, phase) = phase_work(&fixture);
    let exact = setup + 4 * phase;
    let baseline = certificate_for(&fixture);
    assert_eq!(baseline.deletion_witness_work(), exact);

    let mut exact_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 4,
                max_deletion_witness_work: exact,
            },
            &mut exact_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let mut one_short_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 4,
                max_deletion_witness_work: exact - 1,
            },
            &mut one_short_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 4,
            certified_deletion_witnesses: 3,
            deletion_witness_work,
            ..
        } if deletion_witness_work == setup + 3 * phase
    ));
}

#[derive(Default)]
struct RecordingObserver {
    progress: Vec<BoundedSemanticMusProgressV1>,
}

impl BoundedSemanticMusObserverV1 for RecordingObserver {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.progress.push(progress);
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[test]
fn entry_phase_boundaries_and_prepublish_honor_cancel_and_deadline() {
    let fixture = anchored_mirror_fixture(2.0, true).fixture;
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, phase) = phase_work(&fixture);
    let exact = setup + 4 * phase;
    let mut recording = RecordingObserver::default();
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1::default(),
            &mut recording,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));

    let entry = recording
        .progress
        .iter()
        .position(|progress| {
            progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == setup
        })
        .expect("first deletion entry")
        + 1;
    let middle = recording
        .progress
        .iter()
        .enumerate()
        .filter_map(|(index, progress)| {
            (progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == setup + phase)
                .then_some(index + 1)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        middle.len(),
        2,
        "phase needs pre- and post-work checkpoints"
    );
    let prepublish = recording
        .progress
        .iter()
        .position(|progress| {
            progress.deletion_witness_checks == 4
                && progress.certified_deletion_witnesses == 4
                && progress.deletion_witness_work == exact
        })
        .expect("prepublication checkpoint")
        + 1;

    for stop_at in [entry, middle[0], middle[1], prepublish] {
        for (control, expected_reason) in [
            (
                BoundedSemanticMusObserverControlV1::Cancelled,
                BoundedSemanticMusUnknownReasonV1::Cancelled,
            ),
            (
                BoundedSemanticMusObserverControlV1::DeadlineReached,
                BoundedSemanticMusUnknownReasonV1::DeadlineReached,
            ),
        ] {
            let mut observer = StopAtCheckpoint {
                calls: 0,
                stop_at,
                control,
            };
            assert!(matches!(
                certify_bounded_current_runtime_semantic_mus_with_observer_v1(
                    &prepared,
                    BoundedSemanticMusLimitsV1::default(),
                    &mut observer,
                ),
                BoundedCurrentRuntimeSemanticMusV1::Unknown { reason, .. }
                    if reason == expected_reason
            ));
        }
    }
}

#[test]
fn constraint_count_boundaries_are_four_eight_sixteen_and_fail_closed_at_seventeen() {
    for count in [4, 8, 16] {
        let mut fixture = anchored_mirror_fixture(2.0, true);
        while fixture.fixture.records.len() < count {
            fixture
                .fixture
                .records
                .push(record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.axis_edge,
                }));
        }
        let certificate = certificate_for(&fixture.fixture);
        assert_eq!(certificate.constraint_ids().len(), 4);
        assert_only_mirror_phase(&certificate);
    }

    let mut oversized = anchored_mirror_fixture(2.0, true);
    while oversized.fixture.records.len() < 17 {
        oversized
            .fixture
            .records
            .push(record(GeometricConstraintKindV1::Horizontal {
                edge: oversized.axis_edge,
            }));
    }
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &oversized.fixture.pattern,
            oversized.fixture.records.iter().cloned(),
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            direct_oracle_calls: 0,
            ..
        }
    ));
}

#[test]
fn storage_order_duplicate_direction_and_extreme_lengths_are_bounded() {
    for length in [f64::MIN_POSITIVE, 2.0, f64::MAX] {
        let mut fixture = anchored_mirror_fixture(length, true);
        fixture.fixture.pattern.vertices.reverse();
        fixture.fixture.pattern.edges.reverse();
        fixture.fixture.records.reverse();
        assert_only_mirror_phase(&certificate_for(&fixture.fixture));
    }

    let mut duplicate = anchored_mirror_fixture(2.0, true);
    duplicate
        .fixture
        .records
        .push(record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: duplicate.raw_target,
            second_vertex: duplicate.raw_source,
            axis_edge: duplicate.axis_edge,
        }));
    duplicate.fixture.records.reverse();
    let certificate = certificate_for(&duplicate.fixture);
    assert_eq!(certificate.constraint_ids().len(), 4);
    assert_only_mirror_phase(&certificate);

    let subnormal = anchored_mirror_fixture(f64::from_bits(1), true);
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &subnormal.fixture.pattern,
            subnormal.fixture.records.iter().cloned(),
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids,
            ..
        } if direct_core_constraint_ids.len() == 4
    ));
}

#[test]
fn exact_overlay_storage_ceiling_is_admitted_and_one_over_fails_closed() {
    let maximum =
        crate::constraint_semantic_mus::MAX_ANCHORED_MIRROR_RESIDUAL_ONLY_OVERLAY_VERTICES_V1;
    let mut exact = anchored_mirror_fixture(2.0, true);
    while exact.fixture.pattern.vertices.len() < maximum {
        exact.fixture.pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(64.0, 64.0),
        });
    }
    assert_only_mirror_phase(&certificate_for(&exact.fixture));

    let mut over = anchored_mirror_fixture(2.0, true);
    while over.fixture.pattern.vertices.len() <= maximum {
        over.fixture.pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(64.0, 64.0),
        });
    }
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &over.fixture.pattern,
            over.fixture.records.iter().cloned(),
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown { .. }
    ));
}

#[test]
fn generic_and_foreign_mirror_shapes_remain_unknown() {
    let mut generic = anchored_mirror_fixture(2.0, true);
    generic.fixture.records = vec![
        generic.fixture.records[0].clone(),
        generic.fixture.records[1].clone(),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: generic.raw_source,
            line_edge: generic.axis_edge,
        }),
    ];
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &generic.fixture.pattern,
            generic.fixture.records.iter().cloned(),
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            ..
        }
    ));

    let mut foreign = anchored_mirror_fixture(2.0, true);
    let foreign_connector = EdgeId::new();
    foreign.fixture.pattern.edges.push(Edge {
        id: foreign_connector,
        start: foreign.axis_start,
        end: foreign.raw_target,
        kind: EdgeKind::Auxiliary,
    });
    foreign.fixture.records[2].constraint = GeometricConstraintKindV1::Horizontal {
        edge: foreign_connector,
    };
    foreign.fixture.records[3].constraint = GeometricConstraintKindV1::Vertical {
        edge: foreign_connector,
    };
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &foreign.fixture.pattern,
            foreign.fixture.records.iter().cloned(),
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            ..
        }
    ));
}
