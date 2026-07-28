use super::*;
use crate::{ConstraintPreflightV1, DirectConstraintConflictKindV1};

#[derive(Clone, Copy)]
pub(super) enum Family {
    RatioCycle,
    GeneralRatioGraph,
    EqualComponent,
}

fn matching_pattern(edge_count: usize) -> (CreasePattern, Vec<EdgeId>) {
    let mut vertices = Vec::with_capacity(edge_count * 2);
    let mut edges = Vec::with_capacity(edge_count);
    let mut edge_records = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        vertices.push(Vertex {
            id: start,
            position: Point2::new(index as f64 * 10.0, index as f64),
        });
        vertices.push(Vertex {
            id: end,
            position: Point2::new(index as f64 * 10.0 + 3.0, index as f64 + 1.0),
        });
        edges.push(edge);
        edge_records.push(Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        });
    }
    (
        CreasePattern {
            vertices,
            edges: edge_records,
        },
        edges,
    )
}

pub(super) fn target_fixtures() -> Vec<(Family, SemanticFixture)> {
    let (cycle_pattern, cycle_edges) = matching_pattern(3);
    let cycle = SemanticFixture {
        pattern: cycle_pattern,
        records: vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: cycle_edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: cycle_edges[1],
                denominator_edge: cycle_edges[0],
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: cycle_edges[2],
                denominator_edge: cycle_edges[1],
                ratio: 3.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: cycle_edges[0],
                denominator_edge: cycle_edges[2],
                ratio: 0.25,
            }),
        ],
    };

    let (graph_pattern, graph_edges) = matching_pattern(3);
    let graph = SemanticFixture {
        pattern: graph_pattern,
        records: vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: graph_edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: graph_edges[1],
                denominator_edge: graph_edges[0],
                ratio: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: graph_edges[2],
                denominator_edge: graph_edges[1],
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: graph_edges[1],
                denominator_edge: graph_edges[2],
                ratio: 0.25,
            }),
        ],
    };

    let (equal_pattern, equal_edges) = matching_pattern(3);
    let equal = SemanticFixture {
        pattern: equal_pattern,
        records: vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: equal_edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: equal_edges[0],
                second_edge: equal_edges[1],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: equal_edges[1],
                second_edge: equal_edges[2],
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: equal_edges[2],
                length_mm: 2.0,
            }),
        ],
    };
    vec![
        (Family::RatioCycle, cycle),
        (Family::GeneralRatioGraph, graph),
        (Family::EqualComponent, equal),
    ]
}

fn has_family(preflight: &ConstraintPreflightV1, family: Family) -> bool {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return false;
    };
    conflicts.iter().any(|conflict| {
        matches!(
            (family, conflict.conflict()),
            (
                Family::RatioCycle,
                DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength { .. }
            ) | (
                Family::GeneralRatioGraph,
                DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength { .. }
            ) | (
                Family::EqualComponent,
                DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent { .. }
            )
        )
    })
}

pub(super) fn length_work_fixture() -> SemanticFixture {
    target_fixtures().remove(0).1
}

#[test]
fn bounded_length_language_promotes_exactly_the_three_target_families() {
    let fixtures = target_fixtures();
    assert_eq!(fixtures.len(), 3);
    for (family, fixture) in fixtures {
        let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(has_family(&prepared.preflight(), family));
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
        assert_eq!(certificate.constraint_ids().len(), 4);
        assert_eq!(certificate.current_assignment_witness_count(), 0);
        assert_eq!(certificate.axis_exactification_witness_count(), 0);
        assert_eq!(
            certificate.single_constraint_constructive_witness_count(),
            0,
        );
        assert_eq!(certificate.pair_constraint_constructive_witness_count(), 0);
        assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
        assert_eq!(
            certificate.length_constraint_constructive_witness_count(),
            4,
        );
    }
}

#[test]
fn shared_endpoint_target_family_remains_unknown() {
    let mut fixture = length_work_fixture();
    let shared = fixture.pattern.edges[0].end;
    fixture.pattern.edges[1].start = shared;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared(
            &fixture.pattern,
            fixture.records,
        )),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            ..
        }
    ));
}
