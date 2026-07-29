use std::collections::BTreeSet;

use super::length_phase;
use super::mirror_phase::anchored_mirror_inventory_fixture;
use super::pair_phase;
use super::singleton_phase::different_fixed_lengths_fixture;
use super::zero_closure_phase::{Provider, zero_closure_fixture};
use super::*;
use crate::{ConstraintPreflightV1, DirectConstraintConflictKindV1};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InventoryFamily {
    DifferentFixedLengths,
    DifferentFixedAngles,
    EqualLengthWithDifferentFixedLengths,
    LengthRatioWithIncompatibleFixedLengths,
    ParallelWithPerpendicularOrientations,
    SameOrientationWithFixedNonParallelAngle,
    PerpendicularOrientationsWithFixedNonRightAngle,
    DifferentRotationalSymmetryAnglesWithFixedRadius,
    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius,
    RotationalSymmetryWithCollinearRadius,
    MirrorSymmetryWithPointOnAxisAndFixedSeparation,
    HorizontalAndVertical,
    DifferentLengthRatios,
    EqualLengthWithNonUnitRatioAndFixedLength,
    NonReciprocalLengthRatiosWithFixedLength,
    NonUnitLengthRatioCycleWithFixedLength,
    InconsistentLengthRatioGraphWithFixedLength,
    InconsistentLengthRatioGraphBetweenFixedLengths,
    DifferentFixedLengthsInEqualLengthComponent,
    PositiveFixedLengthInBoundedZeroLengthClosure,
    ZeroLengthClosureReachesNondegenerateProvider,
}

fn different_fixed_angles_fixture() -> SemanticFixture {
    let vertex = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: first_end,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: second_end,
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: first_edge,
                    start: vertex,
                    end: first_end,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second_edge,
                    start: vertex,
                    end: second_end,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        records: vec![
            record(GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees: 30.0,
            }),
            record(GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees: 120.0,
            }),
        ],
    }
}

fn different_cardinal_rotations_with_fixed_radius_fixture() -> SemanticFixture {
    let center = VertexId::new();
    let source = VertexId::new();
    let target = VertexId::new();
    let radius = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(10.0, 10.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(11.0, 10.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(12.0, 13.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        },
        records: vec![
            record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: center,
                source_vertex: source,
                target_vertex: target,
                angle_degrees: 90.0,
            }),
            record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: center,
                source_vertex: source,
                target_vertex: target,
                angle_degrees: 180.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: radius,
                length_mm: f64::from_bits(1),
            }),
        ],
    }
}

fn noncomplementary_inverse_cardinal_rotations_with_fixed_radius_fixture() -> SemanticFixture {
    let center = VertexId::new();
    let source = VertexId::new();
    let target = VertexId::new();
    let radius = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(10.0, 10.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(11.0, 10.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(12.0, 13.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        },
        records: vec![
            record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: center,
                source_vertex: source,
                target_vertex: target,
                angle_degrees: 90.0,
            }),
            record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: center,
                source_vertex: target,
                target_vertex: source,
                angle_degrees: 180.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: radius,
                length_mm: f64::from_bits(1),
            }),
        ],
    }
}

fn quarter_turn_with_directed_collinear_radius_fixture(angle_degrees: f64) -> SemanticFixture {
    let center = VertexId::new();
    let source = VertexId::new();
    let target = VertexId::new();
    let radius = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(2.0, 1.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        },
        records: vec![
            record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: center,
                source_vertex: source,
                target_vertex: target,
                angle_degrees,
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: target,
                line_edge: radius,
            }),
        ],
    }
}

fn inventory() -> Vec<(InventoryFamily, SemanticFixture)> {
    let mut result = vec![
        (
            InventoryFamily::DifferentFixedLengths,
            different_fixed_lengths_fixture(3.0),
        ),
        (
            InventoryFamily::DifferentFixedAngles,
            different_fixed_angles_fixture(),
        ),
        (
            InventoryFamily::DifferentRotationalSymmetryAnglesWithFixedRadius,
            different_cardinal_rotations_with_fixed_radius_fixture(),
        ),
        (
            InventoryFamily::NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius,
            noncomplementary_inverse_cardinal_rotations_with_fixed_radius_fixture(),
        ),
        (
            InventoryFamily::RotationalSymmetryWithCollinearRadius,
            quarter_turn_with_directed_collinear_radius_fixture(90.0),
        ),
        (
            InventoryFamily::MirrorSymmetryWithPointOnAxisAndFixedSeparation,
            anchored_mirror_inventory_fixture(),
        ),
    ];
    result.extend(
        pair_phase::promoted_fixtures()
            .into_iter()
            .chain(pair_phase::algebraic_pair_fixtures())
            .map(|(family, fixture)| {
                let inventory = match family {
                    pair_phase::Family::EqualDifferentFixed => {
                        InventoryFamily::EqualLengthWithDifferentFixedLengths
                    }
                    pair_phase::Family::RatioIncompatibleFixed => {
                        InventoryFamily::LengthRatioWithIncompatibleFixedLengths
                    }
                    pair_phase::Family::ParallelPerpendicular => {
                        InventoryFamily::ParallelWithPerpendicularOrientations
                    }
                    pair_phase::Family::SameOrientationAngle => {
                        InventoryFamily::SameOrientationWithFixedNonParallelAngle
                    }
                    pair_phase::Family::PerpendicularAngle => {
                        InventoryFamily::PerpendicularOrientationsWithFixedNonRightAngle
                    }
                    pair_phase::Family::HorizontalVertical => {
                        InventoryFamily::HorizontalAndVertical
                    }
                    pair_phase::Family::DifferentRatios => InventoryFamily::DifferentLengthRatios,
                    pair_phase::Family::EqualNonUnitRatio => {
                        InventoryFamily::EqualLengthWithNonUnitRatioAndFixedLength
                    }
                    pair_phase::Family::NonReciprocalRatios => {
                        InventoryFamily::NonReciprocalLengthRatiosWithFixedLength
                    }
                };
                (inventory, fixture)
            }),
    );
    result.extend(
        length_phase::target_fixtures()
            .into_iter()
            .map(|(family, fixture)| {
                let inventory = match family {
                    length_phase::Family::RatioCycle => {
                        InventoryFamily::NonUnitLengthRatioCycleWithFixedLength
                    }
                    length_phase::Family::GeneralRatioGraph => {
                        InventoryFamily::InconsistentLengthRatioGraphWithFixedLength
                    }
                    length_phase::Family::CrossRootRatioGraph => {
                        InventoryFamily::InconsistentLengthRatioGraphBetweenFixedLengths
                    }
                    length_phase::Family::EqualComponent => {
                        InventoryFamily::DifferentFixedLengthsInEqualLengthComponent
                    }
                };
                (inventory, fixture)
            }),
    );
    result.extend([
        (
            InventoryFamily::PositiveFixedLengthInBoundedZeroLengthClosure,
            zero_closure_fixture(Provider::FixedLength, false),
        ),
        (
            InventoryFamily::ZeroLengthClosureReachesNondegenerateProvider,
            zero_closure_fixture(Provider::PointOnLine, false),
        ),
    ]);
    result
}

struct RotationStopAtCheckpoint {
    calls: usize,
    stop_at: usize,
    control: BoundedSemanticMusObserverControlV1,
}

impl BoundedSemanticMusObserverV1 for RotationStopAtCheckpoint {
    fn checkpoint(
        &mut self,
        _progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.calls += 1;
        if self.calls == self.stop_at {
            self.control
        } else {
            BoundedSemanticMusObserverControlV1::Continue
        }
    }
}

#[test]
fn directed_collinear_quarter_turn_semantic_mus_has_two_independent_singleton_witnesses() {
    for angle_degrees in [90.0, 270.0] {
        let fixture = quarter_turn_with_directed_collinear_radius_fixture(angle_degrees);
        for retained in &fixture.records {
            assert!(
                crate::construct_single_constraint_exact_assignment_v1(
                    &fixture.pattern,
                    &document([retained.clone()]),
                )
                .is_some(),
                "deleting either direct-conflict record must leave a constructively exact singleton",
            );
        }
        let forward = prepared(&fixture.pattern, fixture.records.iter().cloned());
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&forward));
        assert_eq!(
            certificate.constraint_ids(),
            sorted_ids(fixture.records.iter().cloned())
        );
        assert_eq!(certificate.deletion_witness_checks(), 2);
        assert_eq!(
            certificate.single_constraint_constructive_witness_count(),
            2
        );

        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.vertices.reverse();
        reversed_pattern.edges.reverse();
        let reversed = prepared(&reversed_pattern, fixture.records.iter().rev().cloned());
        assert_eq!(
            certify_bounded_current_runtime_semantic_mus_v1(&reversed),
            BoundedCurrentRuntimeSemanticMusV1::Certified(certificate),
        );
    }
}

#[test]
fn collinear_rotation_nonquarter_and_rounded_values_stay_unknown_while_overflow_is_rejected() {
    for angle_degrees in [
        180.0,
        45.0,
        90.0_f64.next_down(),
        90.0_f64.next_up(),
        270.0_f64.next_down(),
        270.0_f64.next_up(),
        f64::from_bits(1),
    ] {
        let fixture = quarter_turn_with_directed_collinear_radius_fixture(angle_degrees);
        let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(matches!(
            prepared_set.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: crate::GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
        assert!(matches!(
            certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
            BoundedCurrentRuntimeSemanticMusV1::Unknown {
                reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
                ..
            }
        ));
    }
    let overflow = quarter_turn_with_directed_collinear_radius_fixture(f64::MAX);
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &overflow.pattern,
            &document(overflow.records),
            GeometricConstraintLimitsV1::default(),
        ),
        Err(crate::GeometricConstraintErrorV1::RotationAngleOutOfRange { .. })
    ));

    let mut reversed = quarter_turn_with_directed_collinear_radius_fixture(90.0);
    let radius = &mut reversed.pattern.edges[0];
    (radius.start, radius.end) = (radius.end, radius.start);
    let prepared_set = prepared(&reversed.pattern, reversed.records.iter().cloned());
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            ..
        }
    ));

    let mut wrong_edge = quarter_turn_with_directed_collinear_radius_fixture(90.0);
    let unrelated = VertexId::new();
    wrong_edge.pattern.vertices.push(Vertex {
        id: unrelated,
        position: Point2::new(2.0, 0.0),
    });
    let wrong_line = EdgeId::new();
    let center = wrong_edge.pattern.edges[0].start;
    wrong_edge.pattern.edges.push(Edge {
        id: wrong_line,
        start: center,
        end: unrelated,
        kind: EdgeKind::Auxiliary,
    });
    let GeometricConstraintKindV1::PointOnLine { line_edge, .. } =
        &mut wrong_edge.records[1].constraint
    else {
        unreachable!()
    };
    *line_edge = wrong_line;
    let prepared_set = prepared(&wrong_edge.pattern, wrong_edge.records.iter().cloned());
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            ..
        }
    ));
}

#[test]
fn directed_collinear_quarter_turn_semantic_limits_and_stops_fail_closed() {
    let fixture = quarter_turn_with_directed_collinear_radius_fixture(90.0);
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(
        &prepared_set,
    ));

    let mut exact_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 2,
                max_deletion_witness_work: baseline.deletion_witness_work(),
            },
            &mut exact_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let mut one_short_count = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 1,
                max_deletion_witness_work: baseline.deletion_witness_work(),
            },
            &mut one_short_count,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            ..
        }
    ));
    let mut one_short_work = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 2,
                max_deletion_witness_work: baseline.deletion_witness_work() - 1,
            },
            &mut one_short_work,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            ..
        }
    ));

    let mut counter = RotationStopAtCheckpoint {
        calls: 0,
        stop_at: usize::MAX,
        control: BoundedSemanticMusObserverControlV1::Cancelled,
    };
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1::default(),
            &mut counter,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    assert!(counter.calls > 4);
    for stop_at in [1, counter.calls / 2, counter.calls] {
        for (control, reason) in [
            (
                BoundedSemanticMusObserverControlV1::Cancelled,
                BoundedSemanticMusUnknownReasonV1::Cancelled,
            ),
            (
                BoundedSemanticMusObserverControlV1::DeadlineReached,
                BoundedSemanticMusUnknownReasonV1::DeadlineReached,
            ),
        ] {
            let mut observer = RotationStopAtCheckpoint {
                calls: 0,
                stop_at,
                control,
            };
            assert!(matches!(
                certify_bounded_current_runtime_semantic_mus_with_observer_v1(
                    &prepared_set,
                    BoundedSemanticMusLimitsV1::default(),
                    &mut observer,
                ),
                BoundedCurrentRuntimeSemanticMusV1::Unknown {
                    reason: actual,
                    ..
                } if actual == reason
            ));
            assert_eq!(observer.calls, stop_at);
        }
    }
}

fn preflight_has_family(preflight: &ConstraintPreflightV1, family: InventoryFamily) -> bool {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return false;
    };
    conflicts.iter().any(|conflict| {
        matches!(
            (family, conflict.conflict()),
            (
                InventoryFamily::DifferentFixedLengths,
                DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
            ) | (
                InventoryFamily::DifferentFixedAngles,
                DirectConstraintConflictKindV1::DifferentFixedAngles { .. }
            ) | (
                InventoryFamily::EqualLengthWithDifferentFixedLengths,
                DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths { .. }
            ) | (
                InventoryFamily::LengthRatioWithIncompatibleFixedLengths,
                DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths { .. }
            ) | (
                InventoryFamily::ParallelWithPerpendicularOrientations,
                DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations { .. }
            ) | (
                InventoryFamily::SameOrientationWithFixedNonParallelAngle,
                DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle { .. }
            ) | (
                InventoryFamily::PerpendicularOrientationsWithFixedNonRightAngle,
                DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle {
                    ..
                }
            ) | (
                InventoryFamily::DifferentRotationalSymmetryAnglesWithFixedRadius,
                DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
                    ..
                }
            ) | (
                InventoryFamily::NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius,
                DirectConstraintConflictKindV1::
                    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                        ..
                    }
            ) | (
                InventoryFamily::RotationalSymmetryWithCollinearRadius,
                DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius { .. }
            ) | (
                InventoryFamily::MirrorSymmetryWithPointOnAxisAndFixedSeparation,
                DirectConstraintConflictKindV1::
                    MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                        ..
                    }
            ) | (
                InventoryFamily::HorizontalAndVertical,
                DirectConstraintConflictKindV1::HorizontalAndVertical { .. }
            ) | (
                InventoryFamily::DifferentLengthRatios,
                DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
            ) | (
                InventoryFamily::EqualLengthWithNonUnitRatioAndFixedLength,
                DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength { .. }
            ) | (
                InventoryFamily::NonReciprocalLengthRatiosWithFixedLength,
                DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength { .. }
            ) | (
                InventoryFamily::NonUnitLengthRatioCycleWithFixedLength,
                DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength { .. }
            ) | (
                InventoryFamily::InconsistentLengthRatioGraphWithFixedLength,
                DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength { .. }
            ) | (
                InventoryFamily::InconsistentLengthRatioGraphBetweenFixedLengths,
                DirectConstraintConflictKindV1::
                    InconsistentLengthRatioGraphBetweenFixedLengths {
                        ..
                    }
            ) | (
                InventoryFamily::DifferentFixedLengthsInEqualLengthComponent,
                DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent { .. }
            ) | (
                InventoryFamily::PositiveFixedLengthInBoundedZeroLengthClosure,
                DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
                    ..
                }
            ) | (
                InventoryFamily::ZeroLengthClosureReachesNondegenerateProvider,
                DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider {
                    ..
                }
            )
        )
    })
}

#[test]
fn public_semantic_pipeline_hard_inventory_is_twenty_one_of_twenty_one() {
    const STABLE_WIRE_FAMILY_COUNT_V1: usize = 24;
    let inventory = inventory();
    assert_eq!(inventory.len(), 21);
    assert_eq!(
        STABLE_WIRE_FAMILY_COUNT_V1 - inventory.len(),
        3,
        "exactly three stable wire families remain outside the public semantic inventory",
    );
    assert_eq!(
        inventory
            .iter()
            .map(|(family, _)| *family)
            .collect::<BTreeSet<_>>()
            .len(),
        21,
        "every supported direct family must have one distinct inventory row",
    );

    for (family, fixture) in inventory {
        let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(
            preflight_has_family(&prepared_set.preflight(), family),
            "missing direct family {family:?}",
        );
        let mutation_document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: fixture.records.clone(),
        };
        assert_eq!(
            crate::constraint_solver::
                verify_deterministic_geometric_constraint_mutation_admission_v1(
                    &fixture.pattern,
                    &mutation_document,
                ),
            Err(crate::ConstraintSolveErrorV1::NonConvergent),
            "direct conflict {family:?} must fail closed at the mutation boundary",
        );
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(
            &prepared_set,
        ));
        assert_eq!(
            certificate.constraint_ids(),
            sorted_ids(fixture.records.iter().cloned()),
            "public semantic promotion returned the wrong core for {family:?}",
        );
        assert_eq!(
            certificate.deletion_witness_checks(),
            fixture.records.len(),
            "every immediate deletion needs a fresh witness for {family:?}",
        );
        assert_eq!(
            certificate.current_assignment_witness_count()
                + certificate.axis_exactification_witness_count()
                + certificate.single_constraint_constructive_witness_count()
                + certificate.pair_constraint_constructive_witness_count()
                + certificate.pair_constraint_algebraic_witness_count()
                + certificate.length_constraint_constructive_witness_count()
                + certificate.zero_length_closure_constructive_witness_count()
                + certificate.anchored_mirror_residual_only_witness_count(),
            fixture.records.len(),
            "method counters must total exactly once for {family:?}",
        );
        if matches!(
            family,
            InventoryFamily::DifferentRotationalSymmetryAnglesWithFixedRadius
                | InventoryFamily::NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius
        ) {
            assert_eq!(
                fixture.records.len(),
                3,
                "each promoted rotation family must retain its exact 3-ID semantic MUS",
            );
            assert_eq!(
                certificate.pair_constraint_constructive_witness_count(),
                2,
                "both one-rotation/radius deletions must be independently recertified \
                 by the geometry-valid bounded pair constructor",
            );
            assert_eq!(
                certificate.pair_constraint_algebraic_witness_count(),
                1,
                "the two-rotation collapse must remain isolated in the complete \
                 residual-only algebraic overlay path",
            );
        }
        if family == InventoryFamily::RotationalSymmetryWithCollinearRadius {
            assert_eq!(fixture.records.len(), 2);
            assert_eq!(
                certificate.single_constraint_constructive_witness_count(),
                2,
                "deleting either exact record must independently construct a SAT witness",
            );
        }
        if family == InventoryFamily::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
            assert_eq!(fixture.records.len(), 4);
            assert_eq!(
                certificate.anchored_mirror_residual_only_witness_count(),
                4,
                "all four raw-source anchored cause deletions need dedicated overlays",
            );
        }
    }
}
