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
    MirrorSymmetryWithPointOnAxisAndFixedSeparation,
    HorizontalAndVertical,
    DifferentLengthRatios,
    EqualLengthWithNonUnitRatioAndFixedLength,
    NonReciprocalLengthRatiosWithFixedLength,
    NonUnitLengthRatioCycleWithFixedLength,
    InconsistentLengthRatioGraphWithFixedLength,
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
fn public_semantic_pipeline_hard_inventory_is_nineteen_of_nineteen() {
    const STABLE_WIRE_FAMILY_COUNT_V1: usize = 23;
    let inventory = inventory();
    assert_eq!(inventory.len(), 19);
    assert_eq!(
        STABLE_WIRE_FAMILY_COUNT_V1 - inventory.len(),
        4,
        "exactly four stable wire families remain outside the public semantic inventory",
    );
    assert_eq!(
        inventory
            .iter()
            .map(|(family, _)| *family)
            .collect::<BTreeSet<_>>()
            .len(),
        19,
        "every supported direct family must have one distinct inventory row",
    );

    for (family, fixture) in inventory {
        let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(
            preflight_has_family(&prepared.preflight(), family),
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
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
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
