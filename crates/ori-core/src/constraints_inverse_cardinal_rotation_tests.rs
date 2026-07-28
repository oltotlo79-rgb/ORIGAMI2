use ori_domain::{EdgeKind, Point2};

use super::*;

#[derive(Clone)]
pub(super) struct Fixture {
    pub(super) pattern: CreasePattern,
    pub(super) vertices: [VertexId; 6],
    pub(super) edges: [EdgeId; 5],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(10.0, 10.0),
            // These duplicate coordinates make edge 3 a geometric alias of
            // edge 0. Proof joining must still use exact endpoint IDs.
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        ];
        let edges = std::array::from_fn(|_| EdgeId::new());
        let endpoints = [
            (vertices[0], vertices[1]),
            (vertices[0], vertices[2]),
            (vertices[1], vertices[2]),
            (vertices[4], vertices[5]),
            (vertices[3], vertices[1]),
        ];
        Self {
            pattern: CreasePattern {
                vertices: vertices
                    .into_iter()
                    .zip(positions)
                    .map(|(id, position)| Vertex { id, position })
                    .collect(),
                edges: edges
                    .into_iter()
                    .zip(endpoints)
                    .map(|(id, (start, end))| Edge {
                        id,
                        start,
                        end,
                        kind: EdgeKind::Auxiliary,
                    })
                    .collect(),
            },
            vertices,
            edges,
        }
    }
}

pub(super) fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

pub(super) fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

pub(super) fn rotation(
    fixture: &Fixture,
    center: usize,
    source: usize,
    target: usize,
    angle_degrees: f64,
) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex: fixture.vertices[center],
        source_vertex: fixture.vertices[source],
        target_vertex: fixture.vertices[target],
        angle_degrees,
    }
}

pub(super) fn fixed_length(
    fixture: &Fixture,
    edge: usize,
    length_mm: f64,
) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[edge],
        length_mm,
    }
}

pub(super) fn core_records(
    fixture: &Fixture,
    forward_angle: f64,
    inverse_angle: f64,
    radius_edge: usize,
    radius: f64,
) -> [GeometricConstraintRecordV1; 3] {
    [
        record(rotation(fixture, 0, 1, 2, forward_angle)),
        record(rotation(fixture, 0, 2, 1, inverse_angle)),
        record(fixed_length(fixture, radius_edge, radius)),
    ]
}

pub(super) fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

pub(super) fn inverse_conflicts(
    preflight: &ConstraintPreflightV1,
) -> Vec<&DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return Vec::new();
    };
    conflicts
        .iter()
        .filter(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                        ..
                    }
            )
        })
        .collect()
}

pub(super) fn assert_target(
    preflight: &ConstraintPreflightV1,
    fixture: &Fixture,
    expected_ids: &[ConstraintId],
    expected_radius_edge: usize,
) {
    let conflicts = inverse_conflicts(preflight);
    assert_eq!(conflicts.len(), 1, "exactly one inverse proof is expected");
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
    let DirectConstraintConflictKindV1::
        NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
            center_vertex,
            source_vertex,
            target_vertex,
            fixed_radius_edge,
        } = conflicts[0].conflict()
    else {
        unreachable!("the helper filtered the conflict kind");
    };
    assert_eq!(*center_vertex, fixture.vertices[0]);
    assert!(
        (*source_vertex == fixture.vertices[1] && *target_vertex == fixture.vertices[2])
            || (*source_vertex == fixture.vertices[2] && *target_vertex == fixture.vertices[1])
    );
    assert_eq!(*fixed_radius_edge, fixture.edges[expected_radius_edge]);
}

fn prepare(
    fixture: &Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> ConstraintPreflightV1 {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("inverse-cardinal fixture must prepare")
    .preflight()
}

#[test]
fn every_nonidentity_cardinal_composition_accepts_both_radii_and_finite_scales() {
    let compositions = [
        (90.0, 90.0),
        (90.0, 180.0),
        (180.0, 90.0),
        (180.0, 270.0),
        (270.0, 180.0),
        (270.0, 270.0),
    ];
    for (forward, inverse) in compositions {
        for radius_edge in [0, 1] {
            for radius in [f64::from_bits(1), f64::MIN_POSITIVE, 1.0, f64::MAX] {
                let fixture = Fixture::new();
                let records = core_records(&fixture, forward, inverse, radius_edge, radius);
                let expected = sorted_ids(records.iter().map(|item| item.id));
                assert_target(
                    &prepare(&fixture, records),
                    &fixture,
                    &expected,
                    radius_edge,
                );
            }
        }
    }
}

#[test]
fn identity_cardinal_compositions_remain_solver_required() {
    for (forward, inverse) in [(90.0, 270.0), (180.0, 180.0), (270.0, 90.0)] {
        let fixture = Fixture::new();
        let preflight = prepare(&fixture, core_records(&fixture, forward, inverse, 0, 1.0));
        assert!(inverse_conflicts(&preflight).is_empty());
        assert!(matches!(
            preflight,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
    }
}

#[test]
fn adjacent_noncardinal_subnormal_and_near_full_turn_angles_fail_closed() {
    let mut noncardinal_cases = Vec::new();
    for cardinal in [90.0_f64, 180.0, 270.0] {
        let counterpart = if cardinal == 180.0 { 90.0 } else { 180.0 };
        for adjacent in [cardinal.next_down(), cardinal.next_up()] {
            // Exercise the one-ULP boundary in both the forward and inverse
            // roles. The exact-cardinal counterpart would form a non-identity
            // composition, so silence cannot be explained by the identity
            // composition exclusion.
            noncardinal_cases.push((adjacent, counterpart));
            noncardinal_cases.push((counterpart, adjacent));
        }
    }
    noncardinal_cases.extend([
        (45.0, 180.0),
        (f64::from_bits(1), 180.0),
        (90.0, f64::from_bits(1)),
        (360.0_f64.next_down(), 90.0),
        (90.0, 360.0_f64.next_down()),
    ]);
    for (forward, inverse) in noncardinal_cases {
        let fixture = Fixture::new();
        let preflight = prepare(
            &fixture,
            core_records(&fixture, forward, inverse, 0, f64::MIN_POSITIVE),
        );
        assert!(
            inverse_conflicts(&preflight).is_empty(),
            "{forward:?}/{inverse:?} must not cross the frozen cardinal boundary",
        );
        assert!(
            matches!(
                preflight,
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    ..
                }
            ),
            "{forward:?}/{inverse:?} must remain solver-required",
        );
    }
}

#[test]
fn radius_must_be_consistent_positive_and_join_an_exact_role_pair() {
    let fixture = Fixture::new();
    let cases = [
        vec![
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
        ],
        core_records(&fixture, 90.0, 180.0, 2, 1.0).to_vec(),
        core_records(&fixture, 90.0, 180.0, 3, 1.0).to_vec(),
        vec![
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 3, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 1.0)),
        ],
        vec![
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(fixed_length(&fixture, 0, 1.0)),
        ],
    ];
    for records in cases {
        let preflight = prepare(&fixture, records);
        assert!(inverse_conflicts(&preflight).is_empty());
        assert!(matches!(
            preflight,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
    }

    let inconsistent = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(fixed_length(&fixture, 0, 1.0)),
        record(fixed_length(&fixture, 0, 2.0)),
    ];
    let preflight = prepare(&fixture, inconsistent);
    assert!(inverse_conflicts(&preflight).is_empty());
    assert!(matches!(
        preflight,
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.iter().all(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
            ))
    ));
}

#[test]
fn duplicates_and_input_or_edge_order_choose_the_same_canonical_witness() {
    let fixture = Fixture::new();
    let forward = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 90.0)),
    ];
    let inverse = [
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
    ];
    let fixed = [
        record(fixed_length(&fixture, 1, 1.0)),
        record(fixed_length(&fixture, 1, 1.0)),
    ];
    let expected = sorted_ids([
        forward
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("two forward records"),
        inverse
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("two inverse records"),
        fixed
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("two fixed records"),
    ]);
    let records = forward
        .into_iter()
        .chain(inverse)
        .chain(fixed)
        .collect::<Vec<_>>();
    let baseline = prepare(&fixture, records.clone());
    assert_target(&baseline, &fixture, &expected, 1);

    let mut reversed_records = records;
    reversed_records.reverse();
    let mut reversed_fixture = fixture.clone();
    reversed_fixture.pattern.edges.reverse();
    let reversed = prepare(&reversed_fixture, reversed_records);
    assert_eq!(
        serde_json::to_value(baseline).expect("serialize baseline"),
        serde_json::to_value(reversed).expect("serialize reversed"),
    );
}

#[test]
fn multiple_cardinal_classes_choose_the_canonical_valid_composition_pair() {
    let fixture = Fixture::new();
    let forward_quarter = record(rotation(&fixture, 0, 1, 2, 90.0));
    let forward_three_quarters = record(rotation(&fixture, 0, 1, 2, 270.0));
    let inverse_quarter = record(rotation(&fixture, 0, 2, 1, 90.0));
    let inverse_half = record(rotation(&fixture, 0, 2, 1, 180.0));
    let fixed = record(fixed_length(&fixture, 0, 1.0));

    // 270 + 90 is the only identity composition in this Cartesian product.
    // The emitted witness must be the lexicographically smallest canonical
    // pair among the other three combinations, regardless of record order.
    let mut valid_pairs = [
        sorted_ids([forward_quarter.id, inverse_quarter.id]),
        sorted_ids([forward_quarter.id, inverse_half.id]),
        sorted_ids([forward_three_quarters.id, inverse_half.id]),
    ];
    valid_pairs.sort_unstable_by_key(|pair| (pair[0].canonical_bytes(), pair[1].canonical_bytes()));
    let expected = sorted_ids(
        valid_pairs[0]
            .iter()
            .copied()
            .chain(std::iter::once(fixed.id)),
    );
    let records = [
        forward_three_quarters,
        inverse_quarter,
        fixed,
        forward_quarter,
        inverse_half,
    ];
    assert_target(&prepare(&fixture, records), &fixture, &expected, 0);
}

#[test]
fn deleting_each_of_the_three_causes_withdraws_the_direct_affirmation() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 90.0, 180.0, 0, 1.0);
    for omitted in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != omitted)
            .map(|(_, item)| item.clone());
        assert!(
            inverse_conflicts(&prepare(&fixture, subset)).is_empty(),
            "cause {omitted} must be necessary",
        );
    }
}
