use std::collections::BTreeMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::{
    MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    component_constructive::{
        MAX_BOUNDED_COMPONENT_FULL_PATTERN_CLONES_V1,
        MAX_BOUNDED_COMPONENT_PREPARATION_OR_VERIFICATION_PASSES_V1,
    },
    construct_bounded_singleton_composition_exact_assignment_v1,
};
use crate::{
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    constraint_solver::residual_referenced_vertices_by_record_v1,
};

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

    fn finish(self) -> CreasePattern {
        CreasePattern {
            vertices: self.vertices,
            edges: self.edges,
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
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
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
fn component_constructor_work_envelope_is_frozen() {
    assert_eq!(
        MAX_BOUNDED_COMPONENT_PREPARATION_OR_VERIFICATION_PASSES_V1,
        138,
    );
    assert_eq!(MAX_BOUNDED_COMPONENT_FULL_PATTERN_CLONES_V1, 112);
}

#[test]
fn connected_pair_component_extends_a_larger_document_and_is_order_canonical() {
    let mut fixture = FixtureBuilder::default();
    let pair_start = fixture.vertex(0.0, 0.0);
    let pair_end = fixture.vertex(3.0, 4.0);
    let detached_start = fixture.vertex(10.0, 2.0);
    let detached_end = fixture.vertex(12.0, 5.0);
    let pair_edge = fixture.edge(pair_start, pair_end);
    let detached_edge = fixture.edge(detached_start, detached_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair_edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: pair_edge }),
        record(GeometricConstraintKindV1::Vertical {
            edge: detached_edge,
        }),
    ]);
    let pattern_before = pattern.clone();
    let source_before = source.clone();

    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("the incompatible singleton pair must use its sound pair template");
    assert_eq!(pattern, pattern_before);
    assert_eq!(source, source_before);
    assert_eq!(assignment.certificate().constraint_count(), 3);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source,)
            .expect("the component candidate remains valid")
            .is_some(),
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let mut reordered_source = source.clone();
    reordered_source.constraints.reverse();
    let reordered = construct_bounded_singleton_composition_exact_assignment_v1(
        &reordered_pattern,
        &reordered_source,
    )
    .expect("storage order cannot change component construction");
    assert_eq!(
        position_bits(assignment.pattern()),
        position_bits(reordered.pattern()),
    );
}

#[test]
fn referenced_edge_endpoints_join_records_into_one_pair_component() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let shared = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let detached_start = fixture.vertex(20.0, 0.0);
    let detached_end = fixture.vertex(22.0, 3.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let detached = fixture.edge(detached_start, detached_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: detached }),
    ]);

    let references = residual_referenced_vertices_by_record_v1(&pattern, &source)
        .expect("the validated fixture has complete residual references");
    assert!(references[0].contains(&shared));
    assert!(references[1].contains(&shared));

    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("endpoint sharing must route the two fixed lengths through one pair");
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source,)
            .expect("the endpoint-derived component remains valid")
            .is_some(),
    );
}

#[test]
fn eight_sound_pair_components_certify_at_sixteen_and_seventeen_is_rejected() {
    let mut fixture = FixtureBuilder::default();
    let mut records = Vec::new();
    let mut first_edge = None;
    for ordinal in 0..8 {
        let base = ordinal as f64 * 20.0;
        let start = fixture.vertex(base + 1.0, base + 2.0);
        let end = fixture.vertex(base + 4.0, base + 7.0);
        let edge = fixture.edge(start, end);
        first_edge.get_or_insert(edge);
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: ordinal as f64 + 2.0,
        }));
        records.push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    let pattern = fixture.finish();
    let source = document(records);
    assert_eq!(
        source.constraints.len(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );

    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("eight independent sound pair components must certify");
    assert_eq!(
        assignment.certificate().constraint_count(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );

    let mut seventeen = source.clone();
    seventeen
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal {
            edge: first_edge.expect("at least one pair edge"),
        }));
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &seventeen).is_none(),
        "seventeen records must fail before any constructive authority is attempted",
    );
}

#[test]
fn progressive_components_avoid_an_unreferenced_connector_edge_collapse() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(1.0, 2.0);
    let first_end = fixture.vertex(4.0, 6.0);
    let second_start = fixture.vertex(20.0, 3.0);
    let second_end = fixture.vertex(24.0, 8.0);
    let detached_start = fixture.vertex(40.0, 2.0);
    let detached_end = fixture.vertex(42.0, 5.0);
    let first = fixture.edge(first_start, first_end);
    let second = fixture.edge(second_start, second_end);
    let connector = fixture.edge(first_end, second_end);
    let detached = fixture.edge(detached_start, detached_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: detached }),
    ]);

    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("a later fixed translation must avoid collapsing the connector");
    let connector = assignment
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == connector)
        .expect("connector is retained");
    let start = assignment
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == connector.start)
        .expect("connector start");
    let end = assignment
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == connector.end)
        .expect("connector end");
    assert_ne!(start.position, end.position);
}

#[test]
fn raw_mirror_record_is_preserved_while_a_separate_pair_component_constructs() {
    let mut fixture = FixtureBuilder::default();
    let axis_start = fixture.vertex(-4.0, 2.0);
    let axis_end = fixture.vertex(4.0, 3.0);
    let mut raw_first = fixture.vertex(1.0, 8.0);
    let mut raw_second = fixture.vertex(2.0, -5.0);
    if raw_first.canonical_bytes() < raw_second.canonical_bytes() {
        std::mem::swap(&mut raw_first, &mut raw_second);
    }
    let pair_start = fixture.vertex(20.0, 1.0);
    let pair_end = fixture.vertex(23.0, 5.0);
    let axis = fixture.edge(axis_start, axis_end);
    let pair = fixture.edge(pair_start, pair_end);
    let pattern = fixture.finish();
    let mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: raw_first,
        second_vertex: raw_second,
        axis_edge: axis,
    });
    let source = document([
        mirror,
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: pair }),
    ]);

    assert!(raw_first.canonical_bytes() > raw_second.canonical_bytes());
    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("record sorting must not normalize the raw mirror operands");
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source,)
            .expect("the raw-role full document remains valid")
            .is_some(),
    );
}

#[test]
fn unsupported_or_exhausted_components_never_use_residual_only_collapse_authority() {
    let mut exhausted = FixtureBuilder::default();
    let start = exhausted.vertex(100.0, 100.0);
    let end = exhausted.vertex(101.0, 100.0);
    let target = exhausted.edge(start, end);
    for point in [
        Point2::new(3.0, 0.0),
        Point2::new(19.0, 32.0),
        Point2::new(-13.0, 8.0),
        Point2::new(1027.0, -512.0),
    ] {
        let blocker = exhausted.vertex(point.x, point.y);
        exhausted.edge(end, blocker);
    }
    let first_detached_start = exhausted.vertex(2_000.0, 1.0);
    let first_detached_end = exhausted.vertex(2_002.0, 4.0);
    let second_detached_start = exhausted.vertex(4_000.0, 2.0);
    let second_detached_end = exhausted.vertex(4_003.0, 6.0);
    let first_detached = exhausted.edge(first_detached_start, first_detached_end);
    let second_detached = exhausted.edge(second_detached_start, second_detached_end);
    let exhausted_pattern = exhausted.finish();
    let exhausted_document = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: target,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: first_detached,
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: second_detached,
        }),
    ]);
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &exhausted_pattern,
            &exhausted_document,
        )
        .is_none(),
        "all four fixed translations exhausted by real geometry must fail closed",
    );

    let mut algebraic = FixtureBuilder::default();
    let center = algebraic.vertex(0.0, 0.0);
    let source_vertex = algebraic.vertex(1.0, 0.0);
    let target_vertex = algebraic.vertex(0.0, 1.0);
    let detached_start = algebraic.vertex(10.0, 1.0);
    let detached_end = algebraic.vertex(12.0, 4.0);
    let detached = algebraic.edge(detached_start, detached_end);
    let algebraic_pattern = algebraic.finish();
    let algebraic_document = document([
        record(GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: center,
            source_vertex,
            target_vertex,
            angle_degrees: 60.0,
        }),
        record(GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: center,
            source_vertex,
            target_vertex,
            angle_degrees: 120.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: detached }),
    ]);
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &algebraic_pattern,
            &algebraic_document,
        )
        .is_none(),
        "a residual-only coincident-role overlay is not a valid detached crease-pattern witness",
    );
}

#[test]
fn whole_document_direct_conflict_precedes_a_constructible_pair_component() {
    let mut fixture = FixtureBuilder::default();
    let pair_start = fixture.vertex(0.0, 0.0);
    let pair_end = fixture.vertex(3.0, 4.0);
    let direct_start = fixture.vertex(20.0, 1.0);
    let direct_end = fixture.vertex(24.0, 6.0);
    let pair = fixture.edge(pair_start, pair_end);
    let direct = fixture.edge(direct_start, direct_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: pair }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: direct,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: direct,
            length_mm: 2.0,
        }),
    ]);

    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source).is_none(),
        "a whole-document direct theorem must block detached SAT construction",
    );
}

#[test]
fn three_record_connected_component_without_a_sound_component_template_fails_closed() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let shared = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);

    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &source)
            .expect("the connected source fixture is structurally valid")
            .is_none(),
        "the source geometry must not already be an exact witness",
    );
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source).is_none(),
        "a size-three component cannot borrow authority from one pair subcomponent",
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let mut reordered_source = source.clone();
    reordered_source.constraints.reverse();
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &reordered_pattern,
            &reordered_source,
        )
        .is_none(),
        "unsupported size-three classification must be order independent",
    );
}
