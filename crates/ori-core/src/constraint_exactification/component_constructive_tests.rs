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
    three_record_component_constructive::{
        MAX_PAIR_PLUS_SINGLETON_STAR_LEAF_CLASSIFICATIONS_V1,
        MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1,
        MAX_PAIR_PLUS_SINGLETON_STAR_REFERENCED_VERTICES_V1,
        MAX_THREE_RECORD_COMPONENT_CONSTRUCTIVE_CANDIDATES_V1,
        MAX_THREE_RECORD_COMPONENT_REFERENCED_VERTICES_V1, MAX_TWO_PAIR_CORE_COMBINATIONS_V1,
        MAX_TWO_PAIR_CORE_LEAF_CLASSIFICATIONS_V1, MAX_TWO_PAIR_CORE_STAR_REFERENCED_VERTICES_V1,
        checked_two_pair_core_classification_bounds_v1,
        construct_pair_plus_singleton_leaf_exact_assignment_v1,
        construct_pair_plus_singleton_star_exact_assignment_v1,
        construct_two_pair_core_singleton_star_exact_assignment_v1,
    },
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

fn two_pair_core_star_fixture(
    leaf_count: usize,
) -> (
    CreasePattern,
    Vec<GeometricConstraintRecordV1>,
    [VertexId; 2],
) {
    assert!(leaf_count <= 12);
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let articulation = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let first_core = fixture.edge(first_start, articulation);
    let second_core = fixture.edge(articulation, second_end);
    let mut records = vec![
        record(GeometricConstraintKindV1::Horizontal { edge: first_core }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_core,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second_core }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second_core,
            length_mm: 3.0,
        }),
    ];
    for ordinal in 0..leaf_count {
        let leaf_end = fixture.vertex(20.0 + ordinal as f64, 5.0 + ordinal as f64);
        let leaf = fixture.edge(articulation, leaf_end);
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: leaf,
            length_mm: 4.0 + ordinal as f64,
        }));
    }
    let external_start = fixture.vertex(400.0, 500.0);
    let external_end = fixture.vertex(404.0, 509.0);
    fixture.edge(external_start, external_end);
    (fixture.finish(), records, [external_start, external_end])
}

#[test]
fn component_constructor_work_envelope_is_frozen() {
    assert_eq!(
        MAX_BOUNDED_COMPONENT_PREPARATION_OR_VERIFICATION_PASSES_V1,
        138,
    );
    assert_eq!(MAX_BOUNDED_COMPONENT_FULL_PATTERN_CLONES_V1, 112);
    assert_eq!(MAX_THREE_RECORD_COMPONENT_CONSTRUCTIVE_CANDIDATES_V1, 4);
    assert_eq!(MAX_THREE_RECORD_COMPONENT_REFERENCED_VERTICES_V1, 7);
    assert_eq!(MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1, 120,);
    assert_eq!(MAX_PAIR_PLUS_SINGLETON_STAR_LEAF_CLASSIFICATIONS_V1, 1_680,);
    assert_eq!(MAX_PAIR_PLUS_SINGLETON_STAR_REFERENCED_VERTICES_V1, 49);
    assert_eq!(MAX_TWO_PAIR_CORE_COMBINATIONS_V1, 7_140);
    assert_eq!(MAX_TWO_PAIR_CORE_LEAF_CLASSIFICATIONS_V1, 85_680);
    assert_eq!(MAX_TWO_PAIR_CORE_STAR_REFERENCED_VERTICES_V1, 49);
    assert_eq!(
        checked_two_pair_core_classification_bounds_v1(16),
        Some((120, 7_140, 85_680)),
    );
    assert_eq!(
        checked_two_pair_core_classification_bounds_v1(4),
        Some((6, 15, 0)),
    );
    assert_eq!(
        checked_two_pair_core_classification_bounds_v1(usize::MAX),
        None,
        "overflowing classification arithmetic must fail closed",
    );

    let mut maximum_passes = 0usize;
    let mut maximum_clones = 0usize;
    for pair_components in 0..=8usize {
        for one_core_components in 0..=5usize {
            for two_core_components in 0..=4usize {
                let occupied_records =
                    pair_components * 2 + one_core_components * 3 + two_core_components * 4;
                if occupied_records > MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 {
                    continue;
                }
                let passes = 5 * MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
                    + 6 * pair_components
                    + 5 * one_core_components
                    + 10 * two_core_components
                    + 8
                    + 2;
                let clones = 4 * MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
                    + 5 * pair_components
                    + 4 * one_core_components
                    + 8 * two_core_components
                    + 8;
                maximum_passes = maximum_passes.max(passes);
                maximum_clones = maximum_clones.max(clones);
            }
        }
    }
    assert_eq!(
        maximum_passes,
        MAX_BOUNDED_COMPONENT_PREPARATION_OR_VERIFICATION_PASSES_V1,
    );
    assert_eq!(maximum_clones, MAX_BOUNDED_COMPONENT_FULL_PATTERN_CLONES_V1,);
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
fn unique_pair_plus_singleton_leaf_component_constructs_in_canonical_order() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let shared = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let external_start = fixture.vertex(40.0, 5.0);
    let external_end = fixture.vertex(44.0, 9.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    fixture.edge(external_start, external_end);
    let pattern = fixture.finish();
    let records = [
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ];
    let source = document(records.clone());

    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &source)
            .expect("the connected source fixture is structurally valid")
            .is_none(),
        "the source geometry must not already be an exact witness",
    );
    let assignment = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("the unique ordinary pair plus singleton leaf must construct");
    assert_eq!(assignment.pattern().edges, pattern.edges);
    for external in [external_start, external_end] {
        let before = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external source vertex");
        let after = assignment
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external candidate vertex");
        assert_eq!(before.position.x.to_bits(), after.position.x.to_bits());
        assert_eq!(before.position.y.to_bits(), after.position.y.to_bits());
    }
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source)
            .expect("the constructed component remains structurally valid")
            .is_some(),
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let reordered_source = document(order.map(|index| records[index].clone()));
        let reordered = construct_bounded_singleton_composition_exact_assignment_v1(
            &reordered_pattern,
            &reordered_source,
        )
        .expect("all six record permutations and reversed storage must construct");
        assert_eq!(
            position_bits(assignment.pattern()),
            position_bits(reordered.pattern()),
        );
    }
}

#[test]
fn unique_pair_plus_two_singleton_leaves_constructs_and_is_order_canonical() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let articulation = fixture.vertex(3.0, 4.0);
    let first_leaf_end = fixture.vertex(8.0, 1.0);
    let second_leaf_end = fixture.vertex(9.0, 7.0);
    let external_start = fixture.vertex(40.0, 5.0);
    let external_end = fixture.vertex(44.0, 9.0);
    let pair_edge = fixture.edge(first_start, articulation);
    let first_leaf = fixture.edge(articulation, first_leaf_end);
    let second_leaf = fixture.edge(articulation, second_leaf_end);
    fixture.edge(external_start, external_end);
    let pattern = fixture.finish();
    let records = [
        record(GeometricConstraintKindV1::Horizontal { edge: pair_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair_edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_leaf,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second_leaf,
            length_mm: 5.0,
        }),
    ];
    let source = document(records.clone());

    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &source)
            .expect("the four-record source fixture is structurally valid")
            .is_none(),
        "the source geometry must not already be an exact witness",
    );
    let assignment = construct_pair_plus_singleton_star_exact_assignment_v1(&pattern, &source)
        .expect("the unique ordinary pair plus two leaves must construct");
    assert_eq!(assignment.certificate().constraint_count(), 4);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(assignment.pattern(), &source)
            .expect("the four-record star remains structurally valid")
            .is_some(),
    );
    for external in [external_start, external_end] {
        let before = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external source vertex");
        let after = assignment
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external candidate vertex");
        assert_eq!(before.position.x.to_bits(), after.position.x.to_bits());
        assert_eq!(before.position.y.to_bits(), after.position.y.to_bits());
    }

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_source = document([
        records[3].clone(),
        records[1].clone(),
        records[2].clone(),
        records[0].clone(),
    ]);
    let reordered = construct_bounded_singleton_composition_exact_assignment_v1(
        &reordered_pattern,
        &reordered_source,
    )
    .expect("record and storage order cannot change the admitted star");
    assert_eq!(
        position_bits(assignment.pattern()),
        position_bits(reordered.pattern()),
    );
}

#[test]
fn pair_plus_leaves_with_a_non_pair_shared_vertex_fail_closed() {
    let mut fixture = FixtureBuilder::default();
    let pair_start = fixture.vertex(0.0, 0.0);
    let articulation = fixture.vertex(3.0, 4.0);
    let shared_outside_pair = fixture.vertex(8.0, 1.0);
    let line_end = fixture.vertex(9.0, 7.0);
    let pair_edge = fixture.edge(pair_start, articulation);
    let first_leaf = fixture.edge(articulation, shared_outside_pair);
    let second_leaf_line = fixture.edge(articulation, line_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::Horizontal { edge: pair_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair_edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_leaf,
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: shared_outside_pair,
            line_edge: second_leaf_line,
        }),
    ]);

    assert!(
        construct_pair_plus_singleton_star_exact_assignment_v1(&pattern, &source).is_none(),
        "leaves coupled away from the admitted pair are not an independent star",
    );
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source).is_none(),
        "the bounded compositor must preserve the non-pair overlap boundary",
    );
}

#[test]
fn connected_pair_plus_fourteen_leaves_reaches_sixteen_and_rejects_seventeen() {
    let mut fixture = FixtureBuilder::default();
    let pair_start = fixture.vertex(0.0, 0.0);
    let articulation = fixture.vertex(3.0, 4.0);
    let pair_edge = fixture.edge(pair_start, articulation);
    let mut records = vec![
        record(GeometricConstraintKindV1::Horizontal { edge: pair_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair_edge,
            length_mm: 2.0,
        }),
    ];
    for ordinal in 0..14 {
        let leaf_end = fixture.vertex(20.0 + ordinal as f64, 5.0 + ordinal as f64);
        let leaf_edge = fixture.edge(articulation, leaf_end);
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: leaf_edge,
            length_mm: 3.0 + ordinal as f64,
        }));
    }
    let pattern = fixture.finish();
    let sixteen = document(records);
    assert_eq!(
        sixteen.constraints.len(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );

    let assignment =
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &sixteen)
            .expect("one pair plus fourteen independent leaves must reach the fixed ceiling");
    assert_eq!(
        assignment.certificate().constraint_count(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            assignment.pattern(),
            &sixteen,
        )
        .expect("the ceiling star remains structurally valid")
        .is_some(),
    );

    let mut seventeen_fixture = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    let seventeenth_end = seventeen_fixture.vertex(99.0, 101.0);
    let seventeenth_edge = seventeen_fixture.edge(articulation, seventeenth_end);
    let seventeen_pattern = seventeen_fixture.finish();
    let mut seventeen = sixteen;
    seventeen
        .constraints
        .push(record(GeometricConstraintKindV1::FixedLength {
            edge: seventeenth_edge,
            length_mm: 17.0,
        }));
    assert!(
        construct_pair_plus_singleton_star_exact_assignment_v1(&seventeen_pattern, &seventeen,)
            .is_none(),
        "the fixed record ceiling must reject a seventeenth connected leaf before classification",
    );
}

#[test]
fn unique_two_pair_cores_construct_at_four_and_are_storage_order_canonical() {
    let (pattern, records, external_vertices) = two_pair_core_star_fixture(0);
    let source = document(records.clone());
    assert_eq!(source.constraints.len(), 4);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &source)
            .expect("the two-core source is structurally valid")
            .is_none(),
        "the source must require detached construction",
    );
    assert!(
        construct_pair_plus_singleton_star_exact_assignment_v1(&pattern, &source).is_none(),
        "the existing one-core family must not absorb two coupled cores",
    );
    let pattern_before = pattern.clone();
    let source_before = source.clone();
    let direct = construct_two_pair_core_singleton_star_exact_assignment_v1(&pattern, &source)
        .expect("the unique two-core decomposition must construct");
    let composed = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source)
        .expect("the component compositor must admit the two-core family");
    assert_eq!(pattern, pattern_before);
    assert_eq!(source, source_before);
    assert_eq!(direct.pattern().edges, pattern.edges);
    assert_eq!(composed.pattern().edges, pattern.edges);
    assert_eq!(
        position_bits(direct.pattern()),
        position_bits(composed.pattern())
    );
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(direct.pattern(), &source)
            .expect("the constructed two-core component remains structurally valid")
            .is_some(),
    );
    for external in external_vertices {
        let before = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external source vertex");
        let after = direct
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == external)
            .expect("external constructed vertex");
        assert_eq!(before.position.x.to_bits(), after.position.x.to_bits());
        assert_eq!(before.position.y.to_bits(), after.position.y.to_bits());
    }

    let baseline = position_bits(composed.pattern());
    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    for order in [[0, 1, 2, 3], [3, 2, 1, 0], [2, 3, 0, 1], [1, 0, 3, 2]] {
        let reordered_source = document(order.map(|index| records[index].clone()));
        let reordered = construct_bounded_singleton_composition_exact_assignment_v1(
            &reordered_pattern,
            &reordered_source,
        )
        .expect("constraint and pattern storage order must not select another decomposition");
        assert_eq!(baseline, position_bits(reordered.pattern()));
    }
}

#[test]
fn two_pair_cores_plus_twelve_leaves_reach_sixteen_and_seventeen_fails_closed() {
    let (pattern, records, _) = two_pair_core_star_fixture(12);
    let sixteen = document(records);
    assert_eq!(
        sixteen.constraints.len(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );
    let assignment = construct_two_pair_core_singleton_star_exact_assignment_v1(&pattern, &sixteen)
        .expect("two cores plus twelve independent leaves must reach sixteen");
    let composed = construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &sixteen)
        .expect("the public compositor must admit the sixteen-record two-core family");
    assert_eq!(
        assignment.certificate().constraint_count(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );
    assert_eq!(
        composed.certificate().constraint_count(),
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
    );
    assert_eq!(
        position_bits(assignment.pattern()),
        position_bits(composed.pattern()),
    );
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            assignment.pattern(),
            &sixteen,
        )
        .expect("the sixteen-record two-core component remains valid")
        .is_some(),
    );

    let articulation = pattern.edges[0].end;
    let mut builder = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    let seventeenth_end = builder.vertex(900.0, 901.0);
    let seventeenth_edge = builder.edge(articulation, seventeenth_end);
    let seventeen_pattern = builder.finish();
    let mut seventeen = sixteen;
    seventeen
        .constraints
        .push(record(GeometricConstraintKindV1::FixedLength {
            edge: seventeenth_edge,
            length_mm: 20.0,
        }));
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(&seventeen_pattern, &seventeen,)
            .is_none(),
        "the two-core classifier must reject seventeen before enumeration",
    );
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(
            &seventeen_pattern,
            &seventeen,
        )
        .is_none(),
        "the public bounded compositor must preserve the sixteen-record ceiling",
    );
}

#[test]
fn two_pair_core_structure_rejects_non_single_articulations_and_leaf_coupling() {
    let (pattern, records, _) = two_pair_core_star_fixture(0);

    let mut detached_builder = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    let detached_start = detached_builder.vertex(100.0, 101.0);
    let detached_end = detached_builder.vertex(103.0, 105.0);
    let detached = detached_builder.edge(detached_start, detached_end);
    let detached_pattern = detached_builder.finish();
    let mut detached_records = records.clone();
    detached_records.push(record(GeometricConstraintKindV1::Horizontal {
        edge: detached,
    }));
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &detached_pattern,
            &document(detached_records),
        )
        .is_none(),
        "a leaf with no core articulation is outside the connected family",
    );

    let first_start = pattern.edges[0].start;
    let second_end = pattern.edges[1].end;
    let mut two_vertex_builder = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    let two_vertex_leaf = two_vertex_builder.edge(first_start, second_end);
    let two_vertex_pattern = two_vertex_builder.finish();
    let mut two_vertex_records = records.clone();
    two_vertex_records.push(record(GeometricConstraintKindV1::FixedLength {
        edge: two_vertex_leaf,
        length_mm: 6.0,
    }));
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &two_vertex_pattern,
            &document(two_vertex_records),
        )
        .is_none(),
        "a leaf meeting the core union twice is not single-articulation",
    );

    let articulation = pattern.edges[0].end;
    let mut coupled_builder = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    let shared_outside = coupled_builder.vertex(200.0, 201.0);
    let line_end = coupled_builder.vertex(205.0, 209.0);
    let first_leaf = coupled_builder.edge(articulation, shared_outside);
    let second_leaf_line = coupled_builder.edge(articulation, line_end);
    let coupled_pattern = coupled_builder.finish();
    let mut coupled_records = records;
    coupled_records.push(record(GeometricConstraintKindV1::FixedLength {
        edge: first_leaf,
        length_mm: 7.0,
    }));
    coupled_records.push(record(GeometricConstraintKindV1::PointOnLine {
        vertex: shared_outside,
        line_edge: second_leaf_line,
    }));
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &coupled_pattern,
            &document(coupled_records),
        )
        .is_none(),
        "leaves sharing a non-core vertex must fail closed",
    );
}

#[test]
fn two_pair_cores_require_exactly_one_shared_vertex_and_one_unique_decomposition() {
    let mut shared_two = FixtureBuilder::default();
    let first = shared_two.vertex(0.0, 0.0);
    let second = shared_two.vertex(3.0, 4.0);
    let third = shared_two.vertex(8.0, 1.0);
    let fourth = shared_two.vertex(9.0, 7.0);
    let first_edge = shared_two.edge(first, second);
    let second_edge = shared_two.edge(first, third);
    let third_edge = shared_two.edge(second, fourth);
    let shared_two_pattern = shared_two.finish();
    let shared_two_document = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: second_edge,
            second_edge: third_edge,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second_edge,
            length_mm: 3.0,
        }),
    ]);
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &shared_two_pattern,
            &shared_two_document,
        )
        .is_none(),
        "two core unions sharing two vertices must not be admitted",
    );

    let mut ambiguous = FixtureBuilder::default();
    let center = ambiguous.vertex(10.0, 10.0);
    let mut ambiguous_records = Vec::new();
    for ordinal in 0..4 {
        let end = ambiguous.vertex(20.0 + ordinal as f64, 30.0 + ordinal as f64);
        let edge = ambiguous.edge(center, end);
        ambiguous_records.push(record(GeometricConstraintKindV1::Horizontal { edge }));
    }
    let ambiguous_pattern = ambiguous.finish();
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &ambiguous_pattern,
            &document(ambiguous_records),
        )
        .is_none(),
        "multiple complete disjoint pair-core decompositions are ambiguous",
    );

    let (non_finite_pattern, mut non_finite_records, _) = two_pair_core_star_fixture(0);
    match &mut non_finite_records[1].constraint {
        GeometricConstraintKindV1::FixedLength { length_mm, .. } => *length_mm = f64::NAN,
        _ => unreachable!(),
    }
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(
            &non_finite_pattern,
            &document(non_finite_records),
        )
        .is_none(),
        "non-finite input must fail before constructive authority",
    );
}

#[test]
fn all_four_two_pair_core_offsets_can_be_exhausted_by_real_geometry() {
    let (pattern, records, _) = two_pair_core_star_fixture(0);
    let source = document(records);
    let baseline = construct_two_pair_core_singleton_star_exact_assignment_v1(&pattern, &source)
        .expect("unblocked two-core fixture must construct");
    let core_vertices = [
        pattern.edges[0].start,
        pattern.edges[0].end,
        pattern.edges[1].end,
    ];
    assert!(baseline.pattern().vertices.iter().any(|vertex| {
        core_vertices.contains(&vertex.id) && vertex.position == Point2::new(0.0, 0.0)
    }));
    let offsets = [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ];
    let (moving_vertex, base_point) = pattern
        .vertices
        .iter()
        .filter_map(|before| {
            baseline
                .pattern()
                .vertices
                .iter()
                .find(|after| after.id == before.id)
                .filter(|after| {
                    offsets.iter().all(|offset| {
                        before.position
                            != Point2::new(after.position.x + offset.x, after.position.y + offset.y)
                    })
                })
                .map(|after| (before.id, after.position))
        })
        .next()
        .expect("the unsatisfied fixture moves at least one vertex away from all offsets");
    let mut blocked = FixtureBuilder {
        vertices: pattern.vertices.clone(),
        edges: pattern.edges.clone(),
    };
    for offset in offsets {
        let blocker = blocked.vertex(base_point.x + offset.x, base_point.y + offset.y);
        blocked.edge(moving_vertex, blocker);
    }
    let blocked_pattern = blocked.finish();
    assert!(
        construct_two_pair_core_singleton_star_exact_assignment_v1(&blocked_pattern, &source)
            .is_none(),
        "real connector collapses must exhaust exactly the four fixed translations",
    );
}

#[test]
fn ambiguous_pair_plus_leaf_decompositions_fail_closed() {
    let mut fixture = FixtureBuilder::default();
    let center = fixture.vertex(10.0, 10.0);
    let first_end = fixture.vertex(12.0, 11.0);
    let second_end = fixture.vertex(9.0, 14.0);
    let third_end = fixture.vertex(15.0, 8.0);
    let first = fixture.edge(center, first_end);
    let second = fixture.edge(center, second_end);
    let third = fixture.edge(center, third_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::Horizontal { edge: second }),
        record(GeometricConstraintKindV1::Horizontal { edge: third }),
    ]);

    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &source).is_none(),
        "three constructible pair choices are ambiguous and must not acquire authority",
    );
}

#[test]
fn connected_three_record_component_without_an_ordinary_pair_template_fails_closed() {
    let mut fixture = FixtureBuilder::default();
    let center = fixture.vertex(0.0, 0.0);
    let first_end = fixture.vertex(2.0, 0.0);
    let second_end = fixture.vertex(0.0, 2.0);
    let third_end = fixture.vertex(-2.0, 0.0);
    let first = fixture.edge(center, first_end);
    let second = fixture.edge(center, second_end);
    let third = fixture.edge(center, third_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: first,
            second_edge: second,
            angle_degrees: 60.0,
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: second,
            second_edge: third,
            angle_degrees: 60.0,
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: third,
            second_edge: first,
            angle_degrees: 60.0,
        }),
    ]);

    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &source).is_none(),
        "connectivity alone must not be mistaken for a supported decomposition",
    );
}

#[test]
fn pair_and_leaf_sharing_two_vertices_is_not_a_single_articulation_template() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(10.0, 10.0);
    let shared = fixture.vertex(13.0, 14.0);
    let second_end = fixture.vertex(18.0, 9.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: first,
            second_edge: second,
        }),
    ]);

    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &source).is_none(),
        "a two-vertex overlap is not a leaf articulation",
    );
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &source).is_none(),
        "the bounded compositor must preserve the narrow classification",
    );
}

#[test]
fn pair_and_leaf_without_a_shared_vertex_are_not_one_three_record_template() {
    let mut fixture = FixtureBuilder::default();
    let pair_start = fixture.vertex(0.0, 0.0);
    let pair_end = fixture.vertex(3.0, 4.0);
    let leaf_start = fixture.vertex(20.0, 1.0);
    let leaf_end = fixture.vertex(24.0, 6.0);
    let pair = fixture.edge(pair_start, pair_end);
    let leaf = fixture.edge(leaf_start, leaf_end);
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: pair,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: pair }),
        record(GeometricConstraintKindV1::Vertical { edge: leaf }),
    ]);

    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &source).is_none(),
        "a detached singleton is not the narrow connected template",
    );
}

#[test]
fn direct_conflict_and_non_finite_input_precede_three_record_construction() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let shared = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let pattern = fixture.finish();
    let conflict = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);
    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &conflict).is_none(),
        "a direct theorem must fail before decomposition",
    );
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &conflict).is_none(),
    );

    let non_finite = document([
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: f64::NAN,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);
    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &non_finite).is_none(),
        "invalid numeric input must fail during preparation",
    );
}

#[test]
fn all_four_pair_plus_leaf_offsets_can_be_exhausted_by_real_geometry() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(100.0, 100.0);
    let shared = fixture.vertex(103.0, 104.0);
    let second_end = fixture.vertex(108.0, 101.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let canonical_origin = if first_start.canonical_bytes() < shared.canonical_bytes() {
        first_start
    } else {
        shared
    };
    for point in [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ] {
        let blocker = fixture.vertex(point.x, point.y);
        fixture.edge(canonical_origin, blocker);
    }
    let pattern = fixture.finish();
    let source = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ]);

    assert!(
        construct_pair_plus_singleton_leaf_exact_assignment_v1(&pattern, &source).is_none(),
        "each fixed translation collapses one unreferenced blocker edge",
    );
}

#[test]
fn pair_plus_leaf_participates_at_sixteen_and_seventeen_is_rejected() {
    let mut fixture = FixtureBuilder::default();
    let first_start = fixture.vertex(0.0, 0.0);
    let shared = fixture.vertex(3.0, 4.0);
    let second_end = fixture.vertex(8.0, 1.0);
    let repeated_start = fixture.vertex(40.0, 2.0);
    let repeated_end = fixture.vertex(43.0, 7.0);
    let first = fixture.edge(first_start, shared);
    let second = fixture.edge(shared, second_end);
    let repeated = fixture.edge(repeated_start, repeated_end);
    let pattern = fixture.finish();
    let mut records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: first }),
        record(GeometricConstraintKindV1::Vertical { edge: second }),
    ];
    for _ in 0..13 {
        records.push(record(GeometricConstraintKindV1::Horizontal {
            edge: repeated,
        }));
    }
    let sixteen = document(records);
    let assignment =
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &sixteen)
            .expect("the narrow three-record component must remain available at the ceiling");
    assert_eq!(assignment.certificate().constraint_count(), 16);

    let mut seventeen = sixteen.clone();
    seventeen
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal {
            edge: repeated,
        }));
    assert!(
        construct_bounded_singleton_composition_exact_assignment_v1(&pattern, &seventeen).is_none(),
        "the existing sixteen-record ceiling remains unchanged",
    );
}
