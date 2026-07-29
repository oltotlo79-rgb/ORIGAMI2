use ori_domain::{EdgeKind, Point2};
use serde_json::{Value, json};

use super::*;

pub(super) struct Fixture {
    pub(super) pattern: CreasePattern,
    pub(super) vertices: [VertexId; 7],
    pub(super) edges: [EdgeId; 6],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(-1.0, 0.0),
            Point2::new(0.0, -1.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 1.0),
        ];
        let vertex_records = vertices
            .into_iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = std::array::from_fn(|_| EdgeId::new());
        let endpoints = [
            (vertices[0], vertices[1]),
            (vertices[0], vertices[2]),
            (vertices[0], vertices[3]),
            (vertices[0], vertices[4]),
            (vertices[5], vertices[6]),
            (vertices[1], vertices[5]),
        ];
        let edge_records = edges
            .into_iter()
            .zip(endpoints)
            .map(|(id, (start, end))| Edge {
                id,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            })
            .collect();
        Self {
            pattern: CreasePattern {
                vertices: vertex_records,
                edges: edge_records,
            },
            vertices,
            edges,
        }
    }

    pub(super) fn all_kinds(&self) -> Vec<GeometricConstraintKindV1> {
        vec![
            GeometricConstraintKindV1::FixedLength {
                edge: self.edges[0],
                length_mm: 20.0,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: self.vertices[0],
                first_edge: self.edges[0],
                second_edge: self.edges[1],
                angle_degrees: 90.0,
            },
            GeometricConstraintKindV1::Horizontal {
                edge: self.edges[0],
            },
            GeometricConstraintKindV1::Vertical {
                edge: self.edges[1],
            },
            GeometricConstraintKindV1::EqualLength {
                first_edge: self.edges[0],
                second_edge: self.edges[1],
            },
            GeometricConstraintKindV1::Parallel {
                first_edge: self.edges[0],
                second_edge: self.edges[4],
            },
            GeometricConstraintKindV1::PointOnLine {
                vertex: self.vertices[2],
                line_edge: self.edges[5],
            },
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: self.vertices[2],
                second_vertex: self.vertices[4],
                axis_edge: self.edges[0],
            },
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: self.vertices[0],
                source_vertex: self.vertices[1],
                target_vertex: self.vertices[2],
                angle_degrees: 90.0,
            },
            GeometricConstraintKindV1::AngleBisector {
                vertex: self.vertices[0],
                first_edge: self.edges[0],
                second_edge: self.edges[1],
                bisector_edge: self.edges[2],
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: self.edges[0],
                denominator_edge: self.edges[1],
                ratio: 2.0,
            },
        ]
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

pub(super) fn prepare<'pattern>(
    fixture: &'pattern Fixture,
    document: &GeometricConstraintDocumentV1,
) -> Result<GeometricConstraintSetV1<'pattern>, GeometricConstraintErrorV1> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        document,
        GeometricConstraintLimitsV1::default(),
    )
}

fn rotation(
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

fn fixed_length(fixture: &Fixture, edge: usize, length_mm: f64) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[edge],
        length_mm,
    }
}

fn radius_padding(fixture: &Fixture) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    }
}

fn assert_solver_required(preflight: &ConstraintPreflightV1) {
    assert!(matches!(
        preflight,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids,
        } if !unchecked_constraint_ids.is_empty()
    ));
}

fn assert_bounded_direct_oracle_unknown(prepared: &GeometricConstraintSetV1<'_>) {
    assert!(matches!(
        find_bounded_direct_mus_v1(prepared),
        BoundedDirectMusV1::Unknown { .. }
    ));
}

fn assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
    fixture: &Fixture,
    records: &[GeometricConstraintRecordV1],
    edge: EdgeId,
) {
    assert_eq!(records.len(), 3, "the exact proof core has three records");
    let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
    let prepared = prepare(fixture, &document(records.iter().cloned()))
        .expect("same-edge parallel non-degeneracy terminal");
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider {
                        provider_kind: ZeroLengthClosureProviderKindV1::Parallel,
                        provider_edge: edge,
                        forced_zero_edge: edge,
                        horizontal_constraint_count: 1,
                        vertical_constraint_count: 1,
                        zero_propagation_constraint_count: 0,
                    },
                constraint_ids: expected_ids.clone(),
            }],
        }
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable {
        constraint_ids,
        oracle_calls,
    } = find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the parallel non-degeneracy terminal must feed the bounded oracle");
    };
    assert_eq!(constraint_ids, expected_ids);
    assert_eq!(oracle_calls, 7);
    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned();
        assert!(!matches!(
            prepare(fixture, &document(subset))
                .expect("proper same-edge parallel terminal subset")
                .preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }
}

// These helpers inspect both emitted allowlisted conflicts and quarantined
// legacy recognizer output. Each test must separately assert whether its
// family is proof-authoritative or remains solver-required.
fn emitted_and_quarantined_conflicts(
    preflight: &ConstraintPreflightV1,
) -> Vec<DirectConstraintConflictV1> {
    let mut candidates = match preflight {
        ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
        ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
            Vec::new()
        }
    };
    candidates.extend(last_quarantined_direct_conflicts());
    candidates
}

fn rotation_conflicts(preflight: &ConstraintPreflightV1) -> Vec<DirectConstraintConflictV1> {
    let conflicts = match preflight {
        ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
        ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
            Vec::new()
        }
    };
    conflicts
        .into_iter()
        .filter(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    DifferentRotationalSymmetryAnglesWithFixedRadius { .. }
            )
        })
        .collect()
}

fn only_rotation_conflict(
    fixture: &Fixture,
    raw: &GeometricConstraintDocumentV1,
) -> Option<DirectConstraintConflictV1> {
    let prepared = prepare(fixture, raw).expect("rotation fixture prepares");
    let preflight = prepared.preflight();
    let mut found = rotation_conflicts(&preflight);
    (found.len() == 1).then(|| found.remove(0))
}

fn inverse_rotation_conflicts(
    preflight: &ConstraintPreflightV1,
) -> Vec<DirectConstraintConflictV1> {
    let conflicts = match preflight {
        ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
        ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
            Vec::new()
        }
    };
    conflicts
        .into_iter()
        .filter(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius { .. }
            )
        })
        .collect()
}

fn only_inverse_rotation_conflict(
    fixture: &Fixture,
    raw: &GeometricConstraintDocumentV1,
) -> Option<DirectConstraintConflictV1> {
    let prepared = prepare(fixture, raw).expect("inverse rotation fixture prepares");
    let preflight = prepared.preflight();
    let mut found = inverse_rotation_conflicts(&preflight);
    (found.len() == 1).then(|| found.remove(0))
}

fn mirror_axis_conflicts(preflight: &ConstraintPreflightV1) -> Vec<DirectConstraintConflictV1> {
    emitted_and_quarantined_conflicts(preflight)
        .into_iter()
        .filter(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    MirrorSymmetryWithPointOnAxisAndFixedSeparation { .. }
            )
        })
        .collect()
}

fn only_mirror_axis_conflict(
    fixture: &Fixture,
    raw: &GeometricConstraintDocumentV1,
) -> Option<DirectConstraintConflictV1> {
    let prepared = prepare(fixture, raw).expect("mirror-axis fixture prepares");
    let preflight = prepared.preflight();
    assert_solver_required(&preflight);
    let mut found = mirror_axis_conflicts(&preflight);
    (found.len() == 1).then(|| found.remove(0))
}

fn collinear_rotation_conflicts(
    preflight: &ConstraintPreflightV1,
) -> Vec<DirectConstraintConflictV1> {
    emitted_and_quarantined_conflicts(preflight)
        .into_iter()
        .filter(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius { .. }
            )
        })
        .collect()
}

fn only_collinear_rotation_conflict(
    fixture: &Fixture,
    raw: &GeometricConstraintDocumentV1,
) -> Option<DirectConstraintConflictV1> {
    let prepared = prepare(fixture, raw).expect("collinear-rotation fixture prepares");
    let preflight = prepared.preflight();
    let mut found = collinear_rotation_conflicts(&preflight);
    (found.len() == 1).then(|| found.remove(0))
}

fn collinear_rotation_witness_records(
    fixture: &Fixture,
    source_is_line_point: bool,
    angle_degrees: f64,
) -> [GeometricConstraintRecordV1; 2] {
    let (source, target) = if source_is_line_point { (2, 5) } else { (5, 2) };
    [
        record(rotation(fixture, 1, source, target, angle_degrees)),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        }),
    ]
}

#[test]
fn exact_quarter_turn_conflicts_only_for_directed_center_source_radius() {
    for source_is_line_point in [true, false] {
        let fixture = Fixture::new();
        let records = collinear_rotation_witness_records(&fixture, source_is_line_point, 90.0);
        let raw = document(records.clone());
        let conflict = only_collinear_rotation_conflict(&fixture, &raw);
        if source_is_line_point {
            assert!(
                conflict.is_none(),
                "source-on-center-target is outside the directed theorem"
            );
            continue;
        }
        let conflict = conflict.expect("target on directed center-source radius conflicts");
        let (source_vertex, target_vertex) = if source_is_line_point {
            (fixture.vertices[2], fixture.vertices[5])
        } else {
            (fixture.vertices[5], fixture.vertices[2])
        };
        assert_eq!(
            *conflict.conflict(),
            DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                center_vertex: fixture.vertices[1],
                source_vertex,
                target_vertex,
                line_edge: fixture.edges[5],
            }
        );
        assert_eq!(
            conflict.constraint_ids(),
            sorted_ids(&records.map(|record| record.id))
        );

        let prepared = prepare(&fixture, &raw).expect("the exact witness prepares");
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable { .. }
        ));
    }
}

#[test]
fn collinear_rotation_conflict_requires_non_half_turn_and_exact_roles_and_edge() {
    let fixture = Fixture::new();
    let negatives = [
        document(collinear_rotation_witness_records(&fixture, true, 180.0)),
        document([
            record(rotation(&fixture, 1, 2, 5, 90.0)),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[4],
            }),
        ]),
        document([
            record(rotation(&fixture, 1, 2, 5, 90.0)),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[6],
                line_edge: fixture.edges[5],
            }),
        ]),
    ];
    for raw in negatives {
        let preflight = prepare(&fixture, &raw)
            .expect("strict negative fixture prepares")
            .preflight();
        assert!(
            collinear_rotation_conflicts(&preflight).is_empty(),
            "half turns and unrelated roles or edges stay unchecked"
        );
    }

    let irrelevant_fixed_group = document([
        record(rotation(&fixture, 1, 5, 2, 90.0)),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        }),
        record(fixed_length(&fixture, 5, 1.0)),
        record(fixed_length(&fixture, 5, 1.0_f64.next_up())),
    ]);
    let preflight = prepare(&fixture, &irrelevant_fixed_group)
        .expect("bit-distinct positive lengths prepare")
        .preflight();
    assert_eq!(
        collinear_rotation_conflicts(&preflight).len(),
        1,
        "unrelated scalar conflicts neither establish nor suppress the two-ID theorem"
    );
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("both independent direct conflicts remain visible")
    };
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
    )));
}

#[test]
fn collinear_rotation_uses_constraints_not_initial_coordinates_and_rejects_rounded_angles() {
    let fixture = Fixture::new();
    let initially_collinear = document([
        record(rotation(&fixture, 0, 1, 3, 90.0)),
        record(fixed_length(&fixture, 0, 1.0)),
    ]);
    assert_eq!(fixture.pattern.vertices[0].position.y, 0.0);
    assert_eq!(fixture.pattern.vertices[1].position.y, 0.0);
    assert_eq!(fixture.pattern.vertices[3].position.y, 0.0);
    assert!(
        collinear_rotation_conflicts(
            &prepare(&fixture, &initially_collinear)
                .expect("initially collinear geometry prepares")
                .preflight()
        )
        .is_empty(),
        "initial coordinates never replace an exact PointOnLine record"
    );

    for angle in [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        180.0_f64.next_down(),
        180.0_f64.next_up(),
        360.0_f64.next_down(),
    ] {
        let raw = document(collinear_rotation_witness_records(&fixture, false, angle));
        assert!(
            only_collinear_rotation_conflict(&fixture, &raw).is_none(),
            "non-cardinal and boundary-rounded angles remain solver-required"
        );
    }
}

#[test]
fn collinear_rotation_candidate_core_is_canonical_irredundant_and_order_independent() {
    let fixture = Fixture::new();
    let first_rotation = record(rotation(&fixture, 1, 5, 2, 90.0));
    let second_rotation = record(rotation(&fixture, 1, 5, 2, 90.0));
    let first_point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[2],
        line_edge: fixture.edges[5],
    });
    let second_point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[2],
        line_edge: fixture.edges[5],
    });
    let records = vec![
        first_rotation.clone(),
        second_rotation.clone(),
        first_point.clone(),
        second_point.clone(),
    ];
    let expected_ids = [
        [first_rotation.id, second_rotation.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("rotation minimum"),
        [first_point.id, second_point.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("point minimum"),
    ];
    let forward = document(records.clone());
    let forward_preflight = prepare(&fixture, &forward)
        .expect("duplicate witness prepares")
        .preflight();
    let mut found = collinear_rotation_conflicts(&forward_preflight);
    assert_eq!(found.len(), 1);
    assert_eq!(found.remove(0).constraint_ids(), sorted_ids(&expected_ids));

    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.edges.reverse();
    let reversed = document(records.into_iter().rev());
    let reversed_preflight = prepare_geometric_constraints_v1(
        &reversed_pattern,
        &reversed,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("reversed duplicate witness prepares")
    .preflight();
    assert_eq!(
        serde_json::to_value(forward_preflight).unwrap(),
        serde_json::to_value(reversed_preflight).unwrap()
    );

    let minimal = collinear_rotation_witness_records(&fixture, false, 90.0);
    for omitted in 0..minimal.len() {
        let subset = document(
            minimal
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, record)| record.clone()),
        );
        assert!(
            collinear_rotation_conflicts(&prepare(&fixture, &subset).unwrap().preflight())
                .is_empty()
        );
    }
}

#[test]
fn collinear_rotation_join_work_depends_on_unique_point_edge_buckets_not_rotation_count() {
    let fixture = Fixture::new();
    let mut records = Vec::new();
    let roles = [
        (1, 2, 5),
        (5, 2, 1),
        (1, 5, 2),
        (5, 1, 2),
        (0, 2, 1),
        (1, 2, 0),
        (0, 1, 2),
        (1, 0, 2),
    ];
    for _ in 0..24 {
        records.extend(roles.into_iter().map(|(center, source, target)| {
            record(rotation(&fixture, center, source, target, 90.0))
        }));
    }
    records.extend([
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[0],
        }),
    ]);
    let raw = document(records);
    let prepared = prepare(&fixture, &raw).expect("large duplicate bucket prepares");
    begin_point_line_join_visit_count();
    let preflight = prepared.preflight();
    assert_eq!(
        finish_point_line_join_visit_count(),
        2,
        "each indexed (point, edge) bucket is joined once, regardless of rotation count"
    );
    assert_eq!(
        collinear_rotation_conflicts(&preflight).len(),
        2,
        "two distinct matching buckets each emit one canonical conflict; duplicate rotations do not multiply them"
    );
}

#[test]
fn collinear_rotation_conflict_keeps_its_wire_tag_and_stable_sort_rank() {
    let fixture = Fixture::new();
    let raw = document(collinear_rotation_witness_records(&fixture, false, 90.0));
    let conflict = only_collinear_rotation_conflict(&fixture, &raw)
        .expect("collinear-rotation witness exists");
    let value = serde_json::to_value(&conflict).expect("serialize collinear rotation conflict");
    assert_eq!(
        value["conflict"]["kind"],
        "rotational_symmetry_with_collinear_radius"
    );
    assert_eq!(
        value["conflict"]["line_edge"],
        serde_json::to_value(fixture.edges[5]).expect("serialize radius edge")
    );
    assert_eq!(conflict_sort_key(conflict.conflict()).0, 20);
    assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 2);
}

fn mirror_axis_witness_records(
    fixture: &Fixture,
    point_vertex: usize,
) -> [GeometricConstraintRecordV1; 3] {
    [
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[1],
            second_vertex: fixture.vertices[5],
            axis_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[point_vertex],
            line_edge: fixture.edges[1],
        }),
        record(fixed_length(fixture, 5, f64::MIN_POSITIVE)),
    ]
}

#[test]
fn mirrored_point_on_the_same_axis_conflicts_with_positive_fixed_separation() {
    for point_vertex in [1, 5] {
        let fixture = Fixture::new();
        let records = mirror_axis_witness_records(&fixture, point_vertex);
        let raw = document(records.clone());
        let conflict = only_mirror_axis_conflict(&fixture, &raw)
            .expect("either mirrored member on the exact axis forces collapse");
        let (first_vertex, second_vertex) =
            if fixture.vertices[1].canonical_bytes() < fixture.vertices[5].canonical_bytes() {
                (fixture.vertices[1], fixture.vertices[5])
            } else {
                (fixture.vertices[5], fixture.vertices[1])
            };
        assert_eq!(
            *conflict.conflict(),
            DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                first_vertex,
                second_vertex,
                axis_edge: fixture.edges[1],
                fixed_separation_edge: fixture.edges[5],
            }
        );
        assert_eq!(
            conflict.constraint_ids(),
            sorted_ids(&records.map(|record| record.id))
        );
        let prepared = prepare(&fixture, &raw).expect("the exact witness prepares");
        assert_bounded_direct_oracle_unknown(&prepared);
    }
}

#[test]
fn mirror_axis_conflict_requires_exact_axis_vertex_pair_and_pattern_edge() {
    let fixture = Fixture::new();
    let negative_documents = [
        document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[1],
                line_edge: fixture.edges[2],
            }),
            record(fixed_length(&fixture, 5, 5.0)),
        ]),
        document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[6],
                line_edge: fixture.edges[1],
            }),
            record(fixed_length(&fixture, 5, 5.0)),
        ]),
        document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[1],
                line_edge: fixture.edges[1],
            }),
            record(fixed_length(&fixture, 4, 5.0)),
        ]),
        document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[1],
                line_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[5],
            }),
        ]),
    ];
    for raw in negative_documents {
        let prepared = prepare(&fixture, &raw).expect("exact negative fixture prepares");
        assert!(
            mirror_axis_conflicts(&prepared.preflight()).is_empty(),
            "different axes, outside vertices, unrelated edges, and missing fixed lengths stay unknown"
        );
    }
}

#[test]
fn mirror_axis_conflict_never_uses_initial_collinearity_or_approximate_lengths() {
    let fixture = Fixture::new();
    let initially_on_axis = fixture.pattern.vertices[1].position;
    let axis_start = fixture.pattern.vertices[0].position;
    let axis_end = fixture.pattern.vertices[3].position;
    assert_eq!(
        (initially_on_axis.y - axis_start.y) * (axis_end.x - axis_start.x),
        (initially_on_axis.x - axis_start.x) * (axis_end.y - axis_start.y)
    );

    let no_point_constraint = document([
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[1],
            second_vertex: fixture.vertices[5],
            axis_edge: fixture.edges[2],
        }),
        record(fixed_length(&fixture, 5, 5.0)),
    ]);
    let prepared =
        prepare(&fixture, &no_point_constraint).expect("initial collinearity is valid geometry");
    assert!(mirror_axis_conflicts(&prepared.preflight()).is_empty());

    let raw = document([
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[1],
            second_vertex: fixture.vertices[5],
            axis_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[1],
            line_edge: fixture.edges[1],
        }),
        record(fixed_length(&fixture, 5, 5.0)),
        record(fixed_length(&fixture, 5, 5.0_f64.next_up())),
    ]);
    let preflight = prepare(&fixture, &raw)
        .expect("adjacent positive binary64 lengths prepare")
        .preflight();
    assert!(mirror_axis_conflicts(&preflight).is_empty());
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("bit-distinct fixed lengths retain their primary conflict");
    };
    assert!(conflicts.iter().all(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
    )));
}

#[test]
fn mirror_axis_candidate_core_is_canonical_irredundant_and_order_independent() {
    let fixture = Fixture::new();
    let first_mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[5],
        second_vertex: fixture.vertices[1],
        axis_edge: fixture.edges[1],
    });
    let second_mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[1],
        second_vertex: fixture.vertices[5],
        axis_edge: fixture.edges[1],
    });
    let first_point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[1],
        line_edge: fixture.edges[1],
    });
    let second_point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[5],
        line_edge: fixture.edges[1],
    });
    let first_fixed = record(fixed_length(&fixture, 5, 5.0));
    let second_fixed = record(fixed_length(&fixture, 5, 5.0));
    let records = vec![
        first_mirror.clone(),
        second_mirror.clone(),
        first_point.clone(),
        second_point.clone(),
        first_fixed.clone(),
        second_fixed.clone(),
    ];
    let expected_ids = [
        [first_mirror.id, second_mirror.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("mirror minimum"),
        [first_point.id, second_point.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("point minimum"),
        [first_fixed.id, second_fixed.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("fixed minimum"),
    ];
    let forward = document(records.clone());
    let forward_conflict =
        only_mirror_axis_conflict(&fixture, &forward).expect("canonical witness exists");
    assert_eq!(forward_conflict.constraint_ids(), sorted_ids(&expected_ids));

    let mut reversed_records = records;
    reversed_records.reverse();
    let reversed = document(reversed_records);
    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.edges.reverse();
    let reversed_preflight = prepare_geometric_constraints_v1(
        &reversed_pattern,
        &reversed,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("reversed order prepares")
    .preflight();
    assert_eq!(
        serde_json::to_value(prepare(&fixture, &forward).unwrap().preflight()).unwrap(),
        serde_json::to_value(reversed_preflight).unwrap()
    );

    let minimal_records = mirror_axis_witness_records(&fixture, 1);
    for omitted in 0..minimal_records.len() {
        let subset = document(
            minimal_records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, record)| record.clone()),
        );
        assert!(mirror_axis_conflicts(&prepare(&fixture, &subset).unwrap().preflight()).is_empty());
    }
}

#[test]
fn mirror_axis_fixed_separation_selects_the_global_canonical_real_edge() {
    let fixture = Fixture::new();
    let alternate_edge = EdgeId::new();
    let mut pattern = fixture.pattern.clone();
    pattern.edges.push(Edge {
        id: alternate_edge,
        start: fixture.vertices[5],
        end: fixture.vertices[1],
        kind: EdgeKind::Auxiliary,
    });
    let mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[1],
        second_vertex: fixture.vertices[5],
        axis_edge: fixture.edges[1],
    });
    let point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[1],
        line_edge: fixture.edges[1],
    });
    let first_fixed = record(fixed_length(&fixture, 5, 5.0));
    let second_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: alternate_edge,
        length_mm: 7.0,
    });
    let expected = [
        (first_fixed.id, fixture.edges[5]),
        (second_fixed.id, alternate_edge),
    ]
    .into_iter()
    .min_by_key(|(id, edge)| (id.canonical_bytes(), edge.canonical_bytes()))
    .expect("two fixed-separation candidates have a minimum");
    let records = vec![mirror, point, first_fixed, second_fixed];
    let forward = document(records.clone());
    let forward_preflight = prepare_geometric_constraints_v1(
        &pattern,
        &forward,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("duplicate real separation edges prepare")
    .preflight();
    let mut conflicts = mirror_axis_conflicts(&forward_preflight);
    assert_eq!(conflicts.len(), 1);
    let conflict = conflicts.remove(0);
    let DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
        fixed_separation_edge,
        ..
    } = *conflict.conflict()
    else {
        panic!("the filtered conflict has the mirror-axis kind")
    };
    assert_eq!(fixed_separation_edge, expected.1);
    assert!(conflict.constraint_ids().contains(&expected.0));

    pattern.edges.reverse();
    let reversed = document(records.into_iter().rev());
    let reversed_preflight = prepare_geometric_constraints_v1(
        &pattern,
        &reversed,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("reversed duplicate-edge fixture prepares")
    .preflight();
    assert_eq!(
        serde_json::to_value(forward_preflight).unwrap(),
        serde_json::to_value(reversed_preflight).unwrap()
    );
}

#[test]
fn mirror_axis_conflict_serializes_and_keeps_the_new_final_sort_rank() {
    let fixture = Fixture::new();
    let raw = document(mirror_axis_witness_records(&fixture, 1));
    let conflict = only_mirror_axis_conflict(&fixture, &raw).expect("mirror-axis witness exists");
    let value = serde_json::to_value(&conflict).expect("serialize mirror-axis conflict");
    assert_eq!(
        value["conflict"]["kind"],
        "mirror_symmetry_with_point_on_axis_and_fixed_separation"
    );
    assert_eq!(
        value["conflict"]["fixed_separation_edge"],
        serde_json::to_value(fixture.edges[5]).expect("serialize edge ID")
    );
    assert_eq!(conflict_sort_key(conflict.conflict()).0, 19);
    assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 3);
}

#[test]
fn different_rotation_angles_conflict_with_a_center_source_radius() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let conflict =
        only_rotation_conflict(&fixture, &raw).expect("a positive radius forbids the collapse");
    assert_eq!(
        *conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
            center_vertex: fixture.vertices[0],
            source_vertex: fixture.vertices[1],
            target_vertex: fixture.vertices[2],
            fixed_radius_edge: fixture.edges[0],
        }
    );
    assert_eq!(conflict.constraint_ids().len(), 3);
}

#[test]
fn different_rotation_angles_conflict_with_a_center_target_radius() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 1, 5.0)),
    ]);
    let conflict =
        only_rotation_conflict(&fixture, &raw).expect("either radius proves the same collapse");
    assert_eq!(
        *conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
            center_vertex: fixture.vertices[0],
            source_vertex: fixture.vertices[1],
            target_vertex: fixture.vertices[2],
            fixed_radius_edge: fixture.edges[1],
        }
    );
}

#[test]
fn different_rotation_angles_alone_keep_the_zero_radius_escape() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("two rotations prepare");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn distant_current_coordinates_are_never_radius_evidence() {
    let fixture = Fixture::new();
    let center = fixture.pattern.vertices[0].position;
    let source = fixture.pattern.vertices[1].position;
    assert_ne!((center.x, center.y), (source.x, source.y));
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 270.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("distant vertices prepare");
    assert!(rotation_conflicts(&prepared.preflight()).is_empty());
}

#[test]
fn identical_rotation_angles_with_a_radius_do_not_conflict() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("equal angles prepare");
    assert!(rotation_conflicts(&prepared.preflight()).is_empty());
}

#[test]
fn adjacent_binary64_rotation_angles_remain_solver_required() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 90.0_f64.next_up())),
        record(fixed_length(&fixture, 0, f64::MIN_POSITIVE)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("adjacent rotations prepare");
    assert!(rotation_conflicts(&prepared.preflight()).is_empty());
    assert_solver_required(&prepared.preflight());
}

#[test]
fn all_reachable_distinct_cardinal_pairs_accept_either_positive_radius() {
    let fixture = Fixture::new();
    let minimum = f64::from_bits(1);
    for (first_angle, second_angle) in [(90.0, 180.0), (90.0, 270.0), (180.0, 270.0)] {
        for radius_edge in [0, 1] {
            let records = [
                record(rotation(&fixture, 0, 1, 2, first_angle)),
                record(rotation(&fixture, 0, 1, 2, second_angle)),
                record(fixed_length(&fixture, radius_edge, minimum)),
            ];
            let raw = document(records.clone());
            let prepared = prepare(&fixture, &raw).expect("cardinal rotation pair prepares");
            let conflict = only_rotation_conflict(&fixture, &raw)
                .expect("distinct exact cardinal matrices force both radii to zero");
            assert_eq!(
                conflict.constraint_ids(),
                sorted_ids(&records.map(|item| item.id))
            );
            assert!(matches!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::ProvenUnsatisfiable {
                    ref constraint_ids,
                    ..
                } if constraint_ids == conflict.constraint_ids()
            ));
        }
    }
}

#[test]
fn cardinal_one_ulp_neighbors_and_identity_underflow_remain_solver_required() {
    let fixture = Fixture::new();
    for (cardinal, adjacent) in [
        (90.0, 90.0_f64.next_down()),
        (90.0, 90.0_f64.next_up()),
        (180.0, 180.0_f64.next_down()),
        (180.0, 180.0_f64.next_up()),
        (270.0, 270.0_f64.next_down()),
        (270.0, 270.0_f64.next_up()),
        (f64::from_bits(1), f64::from_bits(2)),
    ] {
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, cardinal)),
            record(rotation(&fixture, 0, 1, 2, adjacent)),
            record(fixed_length(&fixture, 0, f64::from_bits(1))),
        ]);
        let prepared = prepare(&fixture, &raw).expect("fail-closed angle pair prepares");
        let preflight = prepared.preflight();
        assert!(
            rotation_conflicts(&preflight).is_empty(),
            "{cardinal:?}/{adjacent:?} must not cross the bit-exact cardinal boundary"
        );
        assert_solver_required(&preflight);
        assert_bounded_direct_oracle_unknown(&prepared);
    }
}

#[test]
fn cardinal_rotation_pair_selects_the_global_canonical_class_witness() {
    let fixture = Fixture::new();
    let rotations = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(rotation(&fixture, 0, 1, 2, 270.0)),
    ];
    let fixed = record(fixed_length(&fixture, 0, 5.0));
    let mut expected_rotations = rotations.iter().map(|record| record.id).collect::<Vec<_>>();
    expected_rotations.sort_unstable_by_key(ConstraintId::canonical_bytes);
    let raw = document(
        rotations
            .iter()
            .cloned()
            .chain(std::iter::once(fixed.clone())),
    );
    let conflict = only_rotation_conflict(&fixture, &raw)
        .expect("three occupied cardinal classes have a canonical pair");
    assert!(conflict.constraint_ids().contains(&expected_rotations[0]));
    assert!(conflict.constraint_ids().contains(&expected_rotations[1]));
    assert!(!conflict.constraint_ids().contains(&expected_rotations[2]));
    assert!(conflict.constraint_ids().contains(&fixed.id));
}

#[test]
fn a_fixed_length_on_an_unrelated_edge_is_not_a_radius() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 4, 5.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("unrelated fixed length prepares");
    assert!(rotation_conflicts(&prepared.preflight()).is_empty());
}

#[test]
fn rotation_roles_must_match_exactly() {
    let fixture = Fixture::new();
    for second in [
        rotation(&fixture, 0, 2, 1, 180.0),
        rotation(&fixture, 3, 1, 2, 180.0),
        rotation(&fixture, 1, 0, 2, 180.0),
    ] {
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(second),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("role permutations prepare");
        assert!(
            rotation_conflicts(&prepared.preflight()).is_empty(),
            "a different role order is a different relation"
        );
    }
}

#[test]
fn rotation_conflict_is_record_and_edge_order_independent() {
    let fixture = Fixture::new();
    let forward = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let mut reversed_records = forward.constraints.clone();
    reversed_records.reverse();
    let reversed = document(reversed_records);
    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.edges.reverse();
    let forward_preflight = prepare(&fixture, &forward)
        .expect("forward order prepares")
        .preflight();
    let reversed_preflight = prepare_geometric_constraints_v1(
        &reversed_pattern,
        &reversed,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("reversed order prepares")
    .preflight();
    assert_eq!(
        serde_json::to_value(&forward_preflight).expect("serialize forward"),
        serde_json::to_value(&reversed_preflight).expect("serialize reversed"),
    );
}

#[test]
fn rotation_conflict_selects_the_global_canonical_radius_witness() {
    let fixture = Fixture::new();
    let source_radius = record(fixed_length(&fixture, 0, 5.0));
    let target_radius = record(fixed_length(&fixture, 1, 7.0));
    let (expected_id, expected_edge) =
        if source_radius.id.canonical_bytes() < target_radius.id.canonical_bytes() {
            (source_radius.id, fixture.edges[0])
        } else {
            (target_radius.id, fixture.edges[1])
        };
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        source_radius.clone(),
        target_radius.clone(),
    ]);
    let conflict = only_rotation_conflict(&fixture, &raw).expect("both radii are candidates");
    let DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
        fixed_radius_edge,
        ..
    } = *conflict.conflict()
    else {
        panic!("the rotation conflict kind is filtered above");
    };
    assert_eq!(fixed_radius_edge, expected_edge);
    assert!(conflict.constraint_ids().contains(&expected_id));
}

#[test]
fn duplicate_equal_fixed_lengths_use_the_canonical_minimum_id() {
    let fixture = Fixture::new();
    let first = record(fixed_length(&fixture, 0, 5.0));
    let second = record(fixed_length(&fixture, 0, 5.0));
    let expected = if first.id.canonical_bytes() < second.id.canonical_bytes() {
        first.id
    } else {
        second.id
    };
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        first.clone(),
        second.clone(),
    ]);
    let conflict = only_rotation_conflict(&fixture, &raw).expect("equal values stay consistent");
    assert!(conflict.constraint_ids().contains(&expected));
}

#[test]
fn an_inconsistent_fixed_length_group_is_not_radius_evidence() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
        record(fixed_length(&fixture, 0, 6.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("inconsistent lengths prepare");
    let preflight = prepared.preflight();
    assert!(rotation_conflicts(&preflight).is_empty());
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("the inconsistent fixed lengths still conflict");
    };
    assert!(conflicts.iter().all(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
    )));
}

#[test]
fn removing_any_rotation_candidate_record_withdraws_that_candidate() {
    let fixture = Fixture::new();
    let records = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ];
    for omitted in 0..records.len() {
        let kept = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != omitted)
            .map(|(_, value)| value.clone());
        let raw = document(kept);
        let prepared = prepare(&fixture, &raw).expect("each pair prepares");
        assert!(
            rotation_conflicts(&prepared.preflight()).is_empty(),
            "removing witness {omitted} must withdraw the affirmation"
        );
    }
}

#[test]
fn proven_rotation_conflict_stays_stable_for_growing_documents() {
    let fixture = Fixture::new();
    let witness_records = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ];
    let mut expected: Option<Vec<ConstraintId>> = None;
    for total in [4_usize, 8, 16] {
        let mut records = witness_records.to_vec();
        while records.len() < total {
            records.push(record(radius_padding(&fixture)));
        }
        let raw = document(records);
        let prepared = prepare(&fixture, &raw).expect("padded documents prepare");
        let conflict = only_rotation_conflict(&fixture, &raw)
            .expect("padding never hides the rotation witness");
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable {
                ref constraint_ids,
                ..
            } if constraint_ids == conflict.constraint_ids()
        ));
        let constraint_ids = conflict.constraint_ids().to_vec();
        assert_eq!(constraint_ids.len(), 3);
        if let Some(previous) = &expected {
            assert_eq!(&constraint_ids, previous);
        } else {
            expected = Some(constraint_ids);
        }
    }
}

#[test]
fn cardinal_rotation_preflight_observer_stops_fail_closed() {
    struct Stop(GeometricConstraintPreflightObserverControlV1);
    impl GeometricConstraintPreflightObserverV1 for Stop {
        fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
            self.0
        }
    }

    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 270.0)),
        record(fixed_length(&fixture, 1, f64::from_bits(1))),
    ]);
    let prepared = prepare(&fixture, &raw).expect("observer fixture prepares");
    for (control, expected) in [
        (
            GeometricConstraintPreflightObserverControlV1::Cancelled,
            GeometricConstraintUnknownReasonV1::Cancelled,
        ),
        (
            GeometricConstraintPreflightObserverControlV1::DeadlineReached,
            GeometricConstraintUnknownReasonV1::DeadlineReached,
        ),
    ] {
        assert!(matches!(
            prepared.preflight_with_observer(&mut Stop(control)),
            ConstraintPreflightV1::Unknown { reason, .. } if reason == expected
        ));
    }
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

#[test]
fn seventeen_records_keep_the_candidate_without_bounded_subset_minimization() {
    let fixture = Fixture::new();
    let mut records = vec![
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ];
    while records.len() < MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 + 1 {
        records.push(record(radius_padding(&fixture)));
    }
    let raw = document(records);
    let prepared = prepare(&fixture, &raw).expect("seventeen records prepare");
    assert!(only_rotation_conflict(&fixture, &raw).is_some());
    assert_eq!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::Unknown { oracle_calls: 0 }
    );
}

#[test]
fn collapsing_every_role_zeroes_both_rotation_residuals() {
    use crate::constraint_solver::{
        ConstraintSolveLimitsV1, solve_geometric_constraints_with_drivers_v1,
    };
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
    ]);
    let collapsed = Point2::new(0.0, 0.0);
    let preview = solve_geometric_constraints_with_drivers_v1(
        &fixture.pattern,
        &raw,
        &[
            (fixture.vertices[0], collapsed),
            (fixture.vertices[1], collapsed),
            (fixture.vertices[2], collapsed),
        ],
        ConstraintSolveLimitsV1::default(),
    )
    .expect("the collapsed assignment satisfies both rotation angles at once");
    assert_eq!(preview.maximum_residual, 0.0);

    // The escape above is exactly why the preflight stays silent until a
    // positive radius rules the collapse out. The affirmation below is
    // never derived from these solver numbers.
    let prepared = prepare(&fixture, &raw).expect("the same document prepares");
    assert!(rotation_conflicts(&prepared.preflight()).is_empty());
}

#[test]
fn the_rotation_conflict_serializes_its_kind_and_four_entities() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 1, 2, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let conflict = only_rotation_conflict(&fixture, &raw).expect("the witness exists");
    let value = serde_json::to_value(&conflict).expect("serialize the rotation conflict");
    assert_eq!(
        value["conflict"],
        json!({
            "kind": "different_rotational_symmetry_angles_with_fixed_radius",
            "center_vertex": fixture.vertices[0],
            "source_vertex": fixture.vertices[1],
            "target_vertex": fixture.vertices[2],
            "fixed_radius_edge": fixture.edges[0],
        })
    );
    let Value::Array(ids) = &value["constraint_ids"] else {
        panic!("the witness serializes an ID array");
    };
    assert_eq!(ids.len(), 3);
    assert_eq!(
        ids.clone(),
        serde_json::to_value(conflict.constraint_ids())
            .expect("serialize witness ids")
            .as_array()
            .expect("witness ids are an array")
            .clone()
    );
}

#[test]
fn inverse_exact_cardinal_nonidentity_composition_conflicts_with_either_radius() {
    let fixture = Fixture::new();
    for radius_edge in [0, 1] {
        let forward = record(rotation(&fixture, 0, 1, 2, 90.0));
        let inverse = record(rotation(&fixture, 0, 2, 1, 180.0));
        let fixed = record(fixed_length(&fixture, radius_edge, f64::MIN_POSITIVE));
        let raw = document([forward.clone(), inverse.clone(), fixed.clone()]);
        let conflict = only_inverse_rotation_conflict(&fixture, &raw)
            .expect("a non-full-turn composition and positive radius are unsatisfiable");
        let (source_vertex, target_vertex) =
            if fixture.vertices[1].canonical_bytes() < fixture.vertices[2].canonical_bytes() {
                (fixture.vertices[1], fixture.vertices[2])
            } else {
                (fixture.vertices[2], fixture.vertices[1])
            };
        assert_eq!(
            *conflict.conflict(),
            DirectConstraintConflictKindV1::
                NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                    center_vertex: fixture.vertices[0],
                    source_vertex,
                    target_vertex,
                    fixed_radius_edge: fixture.edges[radius_edge],
                }
        );
        let mut expected_ids = vec![forward.id, inverse.id, fixed.id];
        canonicalize_constraint_ids(&mut expected_ids);
        assert_eq!(conflict.constraint_ids(), expected_ids);
    }
}

#[test]
fn inverse_rotation_exact_full_turn_is_not_a_direct_conflict() {
    let fixture = Fixture::new();
    for (forward, inverse) in [(90.0, 270.0), (180.0, 180.0)] {
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, forward)),
            record(rotation(&fixture, 0, 2, 1, inverse)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("complementary rotations prepare");
        assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
    }
}

#[test]
fn inverse_rotation_sum_rounded_to_full_turn_is_deliberately_left_unproven() {
    let fixture = Fixture::new();
    let adjacent = 90.0_f64.next_up();
    assert_ne!(adjacent.to_bits(), 90.0_f64.to_bits());
    assert_eq!(
        (adjacent + 270.0).to_bits(),
        360.0_f64.to_bits(),
        "the exact non-360 dyadic sum is absorbed by binary64 rounding"
    );
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, adjacent)),
        record(rotation(&fixture, 0, 2, 1, 270.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("adjacent angles prepare");
    assert!(
        inverse_rotation_conflicts(&prepared.preflight()).is_empty(),
        "a rounded 360 result must fail closed even when the exact sum differs"
    );
}

#[test]
fn inverse_rotation_subnormal_and_near_full_turn_remain_solver_required() {
    let fixture = Fixture::new();
    let first = f64::from_bits(1);
    let second = 360.0_f64.next_down();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, first)),
        record(rotation(&fixture, 0, 2, 1, second)),
        record(fixed_length(&fixture, 0, f64::from_bits(1))),
    ]);
    let prepared = prepare(&fixture, &raw).expect("boundary angles prepare");
    assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
    assert_solver_required(&prepared.preflight());
}

#[test]
fn inverse_rotation_requires_radius_and_exactly_reversed_roles() {
    let fixture = Fixture::new();
    let cases = [
        document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
        ]),
        document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            record(fixed_length(&fixture, 4, 5.0)),
        ]),
        document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 3, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]),
        document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]),
    ];
    for raw in cases {
        let prepared = prepare(&fixture, &raw).expect("negative case prepares");
        assert!(
            inverse_rotation_conflicts(&prepared.preflight()).is_empty(),
            "missing radius, unrelated edge, different center, and same roles must fail closed"
        );
    }
}

#[test]
fn inverse_rotation_zero_radius_is_rejected_before_preflight() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(fixed_length(&fixture, 0, 0.0)),
    ]);
    assert!(matches!(
        prepare(&fixture, &raw),
        Err(GeometricConstraintErrorV1::NonPositiveLength { .. })
    ));
}

#[test]
fn duplicate_equal_radius_constraints_choose_the_canonical_inverse_witness() {
    let fixture = Fixture::new();
    let first = record(fixed_length(&fixture, 0, 5.0));
    let second = record(fixed_length(&fixture, 0, 5.0));
    let expected = [first.id, second.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("two radius constraints have a minimum");
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        first,
        second,
    ]);
    let conflict = only_inverse_rotation_conflict(&fixture, &raw)
        .expect("equal duplicate fixed lengths remain consistent evidence");
    assert!(conflict.constraint_ids().contains(&expected));
    assert_eq!(conflict.constraint_ids().len(), 3);
}

#[test]
fn contradictory_fixed_lengths_are_not_inverse_rotation_radius_evidence() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
        record(fixed_length(&fixture, 0, 6.0)),
    ]);
    let prepared = prepare(&fixture, &raw).expect("contradictory lengths prepare");
    let preflight = prepared.preflight();
    assert!(inverse_rotation_conflicts(&preflight).is_empty());
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("the contradictory fixed lengths still have their own conflict");
    };
    assert!(conflicts.iter().all(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
    )));
}

#[test]
fn inverse_rotation_candidate_core_is_irredundant_and_order_independent() {
    let fixture = Fixture::new();
    let records = [
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ];
    let forward = document(records.clone());
    let mut reversed_records = records.to_vec();
    reversed_records.reverse();
    let reversed = document(reversed_records);
    assert_eq!(
        serde_json::to_value(
            prepare(&fixture, &forward)
                .expect("forward order prepares")
                .preflight()
        )
        .expect("serialize forward preflight"),
        serde_json::to_value(
            prepare(&fixture, &reversed)
                .expect("reverse order prepares")
                .preflight()
        )
        .expect("serialize reverse preflight")
    );
    for omitted in 0..records.len() {
        let raw = document(
            records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, value)| value.clone()),
        );
        let prepared = prepare(&fixture, &raw).expect("each pair prepares");
        assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
    }
}

#[test]
fn inverse_rotation_conflict_serializes_its_distinct_kind() {
    let fixture = Fixture::new();
    let raw = document([
        record(rotation(&fixture, 0, 1, 2, 90.0)),
        record(rotation(&fixture, 0, 2, 1, 180.0)),
        record(fixed_length(&fixture, 0, 5.0)),
    ]);
    let conflict = only_inverse_rotation_conflict(&fixture, &raw).expect("inverse witness exists");
    let value = serde_json::to_value(&conflict).expect("serialize inverse conflict");
    assert_eq!(
        value["conflict"]["kind"],
        "non_complementary_inverse_rotational_symmetry_angles_with_fixed_radius"
    );
    assert_eq!(
        value["conflict"]["fixed_radius_edge"],
        serde_json::to_value(fixture.edges[0]).expect("serialize edge ID")
    );
    assert_eq!(
        value["constraint_ids"]
            .as_array()
            .expect("witness IDs are an array")
            .len(),
        3
    );
}

#[test]
fn all_eleven_constraint_kinds_are_persistable_and_preparable() {
    let fixture = Fixture::new();
    let raw = document(fixture.all_kinds().into_iter().map(record));
    let json = serde_json::to_string(&raw).expect("serialize all constraint kinds");
    let restored: GeometricConstraintDocumentV1 =
        serde_json::from_str(&json).expect("deserialize all constraint kinds");
    assert_eq!(restored, raw);

    let prepared = prepare(&fixture, &restored).expect("all eleven kinds are valid");
    assert_eq!(prepared.model_id(), GEOMETRIC_CONSTRAINT_MODEL_ID_V1);
    assert_eq!(prepared.constraints().len(), 11);

    let value: Value = serde_json::from_str(&json).expect("valid JSON value");
    let kinds = value["constraints"]
        .as_array()
        .expect("constraint array")
        .iter()
        .map(|entry| entry["constraint"]["kind"].as_str().expect("kind"))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "fixed_length",
            "fixed_angle",
            "horizontal",
            "vertical",
            "equal_length",
            "parallel",
            "point_on_line",
            "mirror_symmetry",
            "rotational_symmetry",
            "angle_bisector",
            "length_ratio",
        ]
    );
}

#[test]
fn serde_rejects_unknown_kind_and_unknown_fields() {
    let fixture = Fixture::new();
    let raw = document([record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    })]);
    let mut unknown_kind = serde_json::to_value(&raw).expect("serialize document");
    unknown_kind["constraints"][0]["constraint"]["kind"] = json!("future_constraint");
    assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_kind).is_err());

    let mut unknown_document_field = serde_json::to_value(&raw).expect("serialize document");
    unknown_document_field["future"] = json!(true);
    assert!(
        serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_document_field).is_err()
    );

    let mut unknown_constraint_field = serde_json::to_value(&raw).expect("serialize document");
    unknown_constraint_field["constraints"][0]["constraint"]["future"] = json!(true);
    assert!(
        serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_constraint_field).is_err()
    );
}

#[test]
fn unsupported_version_nil_id_and_duplicate_ids_fail_closed() {
    let fixture = Fixture::new();
    let mut wrong_version = document([]);
    wrong_version.schema_version = 2;
    assert_eq!(
        prepare(&fixture, &wrong_version).expect_err("future schema must fail"),
        GeometricConstraintErrorV1::UnsupportedSchemaVersion {
            actual: 2,
            expected: 1,
        }
    );

    let nil_json = format!(
        r#"{{"schema_version":1,"constraints":[{{"id":"00000000-0000-0000-0000-000000000000","constraint":{{"kind":"horizontal","edge":"{}"}}}}]}}"#,
        uuid_string(fixture.edges[0])
    );
    let nil_document: GeometricConstraintDocumentV1 =
        serde_json::from_str(&nil_json).expect("nil UUID has valid wire syntax");
    assert_eq!(
        prepare(&fixture, &nil_document).expect_err("nil constraint ID must fail"),
        GeometricConstraintErrorV1::NilConstraintId
    );

    let duplicate = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let duplicate_document = document([duplicate.clone(), duplicate.clone()]);
    assert_eq!(
        prepare(&fixture, &duplicate_document).expect_err("duplicate ID must fail"),
        GeometricConstraintErrorV1::DuplicateConstraintId {
            constraint: duplicate.id,
        }
    );
}

#[test]
fn nil_geometry_ids_fail_closed_before_reference_validation() {
    let nil_vertex: VertexId = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
        .expect("nil vertex ID has valid UUID wire syntax");
    let mut nil_vertex_fixture = Fixture::new();
    nil_vertex_fixture.pattern.vertices[0].id = nil_vertex;
    let vertex_document = document([record(GeometricConstraintKindV1::Horizontal {
        edge: nil_vertex_fixture.edges[0],
    })]);
    assert_eq!(
        prepare(&nil_vertex_fixture, &vertex_document).expect_err("nil vertex ID must fail"),
        GeometricConstraintErrorV1::NilVertexId
    );

    let nil_edge: EdgeId = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
        .expect("nil edge ID has valid UUID wire syntax");
    let mut nil_edge_fixture = Fixture::new();
    nil_edge_fixture.pattern.edges[0].id = nil_edge;
    let edge_document = document([record(GeometricConstraintKindV1::Horizontal {
        edge: nil_edge,
    })]);
    assert_eq!(
        prepare(&nil_edge_fixture, &edge_document).expect_err("nil edge ID must fail"),
        GeometricConstraintErrorV1::NilEdgeId
    );
}

#[test]
fn duplicate_and_invalid_geometry_registries_are_rejected_deterministically() {
    let fixture = Fixture::new();
    let referenced = document([record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    })]);

    let mut duplicate_vertex = fixture.pattern.clone();
    duplicate_vertex
        .vertices
        .push(duplicate_vertex.vertices[0].clone());
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &duplicate_vertex,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::DuplicateVertexId { .. })
    ));

    let mut duplicate_edge = fixture.pattern.clone();
    duplicate_edge.edges.push(duplicate_edge.edges[0].clone());
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &duplicate_edge,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::DuplicateEdgeId { .. })
    ));

    let mut non_finite = fixture.pattern.clone();
    non_finite.vertices[0].position.x = f64::NAN;
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &non_finite,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::NonFiniteVertexPosition { .. })
    ));

    let mut missing_endpoint = fixture.pattern.clone();
    missing_endpoint.edges[0].start = VertexId::new();
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &missing_endpoint,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::EdgeEndpointMissing { .. })
    ));

    let mut degenerate_identity = fixture.pattern.clone();
    degenerate_identity.edges[0].end = degenerate_identity.edges[0].start;
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &degenerate_identity,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::DegenerateGeometryEdge { .. })
    ));

    let mut degenerate_position = fixture.pattern.clone();
    degenerate_position.vertices[1].position = degenerate_position.vertices[0].position;
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &degenerate_position,
            &referenced,
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::DegenerateGeometryEdge { .. })
    ));
}

#[test]
fn empty_v1_document_skips_unreferenced_geometry_but_first_constraint_enforces_the_cap() {
    let repeated = Vertex {
        id: VertexId::new(),
        position: Point2::new(f64::NAN, 0.0),
    };
    let oversized = CreasePattern {
        vertices: vec![repeated; DEFAULT_MAX_CONSTRAINT_VERTICES + 1],
        edges: Vec::new(),
    };
    let empty = document([]);
    let prepared = prepare_geometric_constraints_v1(
        &oversized,
        &empty,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("an empty document has no geometry references to validate");
    assert!(prepared.is_for_pattern(&oversized));
    assert!(prepared.constraints().is_empty());
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::NoDirectConflict
    );

    let first_constraint = document([record(GeometricConstraintKindV1::Horizontal {
        edge: EdgeId::new(),
    })]);
    assert_eq!(
        prepare_geometric_constraints_v1(
            &oversized,
            &first_constraint,
            GeometricConstraintLimitsV1::default(),
        )
        .expect_err("the first constraint activates the shared geometry ceiling"),
        GeometricConstraintErrorV1::ResourceLimitExceeded {
            resource: GeometricConstraintResourceV1::Vertices,
            actual: DEFAULT_MAX_CONSTRAINT_VERTICES + 1,
            maximum: DEFAULT_MAX_CONSTRAINT_VERTICES,
        }
    );

    let mut future_empty = empty;
    future_empty.schema_version += 1;
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &oversized,
            &future_empty,
            GeometricConstraintLimitsV1::default(),
        ),
        Err(GeometricConstraintErrorV1::UnsupportedSchemaVersion { .. })
    ));
}

#[test]
fn missing_vertex_and_edge_references_are_rejected() {
    let fixture = Fixture::new();
    let missing_edge = EdgeId::new();
    let edge_record = record(GeometricConstraintKindV1::FixedLength {
        edge: missing_edge,
        length_mm: 1.0,
    });
    assert_eq!(
        prepare(&fixture, &document([edge_record.clone()])).expect_err("missing edge must fail"),
        GeometricConstraintErrorV1::MissingEdge {
            constraint: edge_record.id,
            role: ConstraintEdgeRoleV1::Target,
            edge: missing_edge,
        }
    );

    let missing_vertex = VertexId::new();
    let vertex_record = record(GeometricConstraintKindV1::PointOnLine {
        vertex: missing_vertex,
        line_edge: fixture.edges[5],
    });
    assert_eq!(
        prepare(&fixture, &document([vertex_record.clone()]))
            .expect_err("missing vertex must fail"),
        GeometricConstraintErrorV1::MissingVertex {
            constraint: vertex_record.id,
            role: ConstraintVertexRoleV1::Point,
            vertex: missing_vertex,
        }
    );
}

#[test]
fn self_references_and_degenerate_semantic_references_are_rejected() {
    let fixture = Fixture::new();
    for constraint in [
        GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[0],
        },
        GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[1],
        },
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[2],
            ratio: 1.0,
        },
    ] {
        assert!(matches!(
            prepare(&fixture, &document([record(constraint)])),
            Err(GeometricConstraintErrorV1::RepeatedEdgeReference { .. })
        ));
    }

    assert!(matches!(
        prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[0],
                target_vertex: fixture.vertices[2],
                angle_degrees: 90.0,
            })])
        ),
        Err(GeometricConstraintErrorV1::RepeatedVertexReference { .. })
    ));

    assert!(matches!(
        prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[1],
                line_edge: fixture.edges[0],
            })])
        ),
        Err(GeometricConstraintErrorV1::PointIsLineEndpoint { .. })
    ));

    assert!(matches!(
        prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[0],
                second_vertex: fixture.vertices[2],
                axis_edge: fixture.edges[0],
            })])
        ),
        Err(GeometricConstraintErrorV1::SymmetryPointIsAxisEndpoint { .. })
    ));

    assert!(matches!(
        prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[6],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                angle_degrees: 90.0,
            })])
        ),
        Err(GeometricConstraintErrorV1::VertexNotIncidentToEdge { .. })
    ));
}

#[test]
fn distinct_ids_at_coincident_geometry_are_degenerate_references() {
    let fixture = Fixture::new();

    let coincident_edge = EdgeId::new();
    let mut duplicate_carrier_pattern = fixture.pattern.clone();
    duplicate_carrier_pattern.edges.push(Edge {
        id: coincident_edge,
        start: fixture.vertices[1],
        end: fixture.vertices[0],
        kind: EdgeKind::Auxiliary,
    });
    let carrier_constraint = record(GeometricConstraintKindV1::EqualLength {
        first_edge: fixture.edges[0],
        second_edge: coincident_edge,
    });
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &duplicate_carrier_pattern,
            &document([carrier_constraint]),
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::CoincidentEdgeReferences { .. })
    ));

    let coincident_vertex = VertexId::new();
    let mut duplicate_position_pattern = fixture.pattern.clone();
    duplicate_position_pattern.vertices.push(Vertex {
        id: coincident_vertex,
        position: duplicate_position_pattern.vertices[1].position,
    });
    let rotation = record(GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex: fixture.vertices[0],
        source_vertex: fixture.vertices[1],
        target_vertex: coincident_vertex,
        angle_degrees: 90.0,
    });
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &duplicate_position_pattern,
            &document([rotation]),
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::CoincidentVertexReferences { .. })
    ));

    let endpoint_alias = VertexId::new();
    duplicate_position_pattern.vertices.push(Vertex {
        id: endpoint_alias,
        position: duplicate_position_pattern.vertices[1].position,
    });
    let point_on_line = record(GeometricConstraintKindV1::PointOnLine {
        vertex: endpoint_alias,
        line_edge: fixture.edges[0],
    });
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &duplicate_position_pattern,
            &document([point_on_line]),
            GeometricConstraintLimitsV1::default()
        ),
        Err(GeometricConstraintErrorV1::PointIsLineEndpoint { .. })
    ));
}

#[test]
fn every_scalar_family_rejects_non_finite_values() {
    let fixture = Fixture::new();
    let cases = [
        (
            GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: f64::INFINITY,
            },
            ConstraintScalarFieldV1::LengthMillimetres,
        ),
        (
            GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                angle_degrees: f64::NEG_INFINITY,
            },
            ConstraintScalarFieldV1::AngleDegrees,
        ),
        (
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[1],
                target_vertex: fixture.vertices[2],
                angle_degrees: f64::NAN,
            },
            ConstraintScalarFieldV1::RotationAngleDegrees,
        ),
        (
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: f64::INFINITY,
            },
            ConstraintScalarFieldV1::Ratio,
        ),
    ];
    for (constraint, expected_field) in cases {
        assert!(matches!(
            prepare(&fixture, &document([record(constraint)])),
            Err(GeometricConstraintErrorV1::NonFiniteValue {
                field,
                ..
            }) if field == expected_field
        ));
    }
}

#[test]
fn scalar_boundary_matrix_is_fail_closed() {
    let fixture = Fixture::new();
    for (length_mm, valid) in [
        (-f64::MIN_POSITIVE, false),
        (-0.0, false),
        (0.0, false),
        (f64::MIN_POSITIVE, true),
        (f64::MAX, true),
    ] {
        let result = prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm,
            })]),
        );
        assert_eq!(result.is_ok(), valid, "length {length_mm:?}");
    }
    for (angle_degrees, valid) in [
        (-f64::MIN_POSITIVE, false),
        (-0.0, true),
        (0.0, true),
        (180.0, true),
        (180.0_f64.next_up(), false),
    ] {
        let result = prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                angle_degrees,
            })]),
        );
        assert_eq!(result.is_ok(), valid, "angle {angle_degrees:?}");
    }
    for (angle_degrees, valid) in [
        (0.0, false),
        (f64::MIN_POSITIVE, true),
        (360.0_f64.next_down(), true),
        (360.0, false),
    ] {
        let result = prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[1],
                target_vertex: fixture.vertices[2],
                angle_degrees,
            })]),
        );
        assert_eq!(result.is_ok(), valid, "rotation {angle_degrees:?}");
    }
    for (ratio, valid) in [
        (-1.0, false),
        (-0.0, false),
        (0.0, false),
        (f64::MIN_POSITIVE, true),
        (f64::MAX, true),
    ] {
        let result = prepare(
            &fixture,
            &document([record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio,
            })]),
        );
        assert_eq!(result.is_ok(), valid, "ratio {ratio:?}");
    }
}

#[test]
fn resource_limits_cover_geometry_constraints_references_and_preflight() {
    let fixture = Fixture::new();
    let one = document([record(GeometricConstraintKindV1::AngleBisector {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        bisector_edge: fixture.edges[2],
    })]);
    let exact_limits = GeometricConstraintLimitsV1 {
        max_vertices: fixture.pattern.vertices.len(),
        max_edges: fixture.pattern.edges.len(),
        max_constraints: 1,
        max_references: 4,
        max_preflight_checks: 1,
    };
    prepare_geometric_constraints_v1(&fixture.pattern, &one, exact_limits)
        .expect("every resource limit admits exact equality");

    for (resource, limits) in [
        (
            GeometricConstraintResourceV1::Vertices,
            GeometricConstraintLimitsV1 {
                max_vertices: fixture.pattern.vertices.len() - 1,
                ..Default::default()
            },
        ),
        (
            GeometricConstraintResourceV1::Edges,
            GeometricConstraintLimitsV1 {
                max_edges: fixture.pattern.edges.len() - 1,
                ..Default::default()
            },
        ),
        (
            GeometricConstraintResourceV1::Constraints,
            GeometricConstraintLimitsV1 {
                max_constraints: 0,
                ..Default::default()
            },
        ),
        (
            GeometricConstraintResourceV1::References,
            GeometricConstraintLimitsV1 {
                max_references: 3,
                ..Default::default()
            },
        ),
    ] {
        assert!(matches!(
            prepare_geometric_constraints_v1(&fixture.pattern, &one, limits),
            Err(GeometricConstraintErrorV1::ResourceLimitExceeded {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let prepared = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &one,
        GeometricConstraintLimitsV1 {
            max_preflight_checks: 0,
            ..Default::default()
        },
    )
    .expect("preflight work limit is represented as Unknown");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
    ));
}

#[test]
fn preflight_defaults_use_the_domain_shared_geometry_hard_ceilings() {
    let limits = GeometricConstraintLimitsV1::default();
    assert_eq!(
        limits.max_vertices,
        ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES
    );
    assert_eq!(limits.max_edges, ori_domain::DEFAULT_MAX_CONSTRAINT_EDGES);
    assert_eq!(
        DEFAULT_MAX_CONSTRAINT_VERTICES,
        ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES
    );
    assert_eq!(
        DEFAULT_MAX_CONSTRAINT_EDGES,
        ori_domain::DEFAULT_MAX_CONSTRAINT_EDGES
    );
}

#[test]
fn caller_limits_can_tighten_but_cannot_relax_v1_hard_ceilings() {
    let fixture = Fixture::new();
    let records = (0..=DEFAULT_MAX_CONSTRAINT_RECORDS)
        .map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            })
        })
        .collect::<Vec<_>>();
    let mut over_ceiling = document(records);
    let relaxed = GeometricConstraintLimitsV1 {
        max_vertices: usize::MAX,
        max_edges: usize::MAX,
        max_constraints: usize::MAX,
        max_references: usize::MAX,
        max_preflight_checks: usize::MAX,
    };
    assert_eq!(
        prepare_geometric_constraints_v1(&fixture.pattern, &over_ceiling, relaxed,)
            .expect_err("caller limits must not relax the V1 hard ceiling"),
        GeometricConstraintErrorV1::ResourceLimitExceeded {
            resource: GeometricConstraintResourceV1::Constraints,
            actual: DEFAULT_MAX_CONSTRAINT_RECORDS + 1,
            maximum: DEFAULT_MAX_CONSTRAINT_RECORDS,
        }
    );

    over_ceiling
        .constraints
        .pop()
        .expect("fixture has exactly one record beyond the ceiling");
    let exact = prepare_geometric_constraints_v1(&fixture.pattern, &over_ceiling, relaxed)
        .expect("the non-relaxable V1 hard ceiling admits exact equality");
    assert_eq!(exact.constraints().len(), DEFAULT_MAX_CONSTRAINT_RECORDS);

    assert_eq!(relaxed.effective(), GeometricConstraintLimitsV1::default());
    let tightened = GeometricConstraintLimitsV1 {
        max_vertices: 1,
        max_edges: 2,
        max_constraints: 3,
        max_references: 4,
        max_preflight_checks: 5,
    };
    assert_eq!(tightened.effective(), tightened);
}

#[test]
fn equal_length_ratio_rounded_residual_conflict_is_irredundant() {
    let fixture = Fixture::new();
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 10.0,
    });
    let equal = record(GeometricConstraintKindV1::EqualLength {
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
    });
    let ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let records = [fixed.clone(), equal.clone(), ratio.clone()];
    let prepared = prepare(&fixture, &document(records.clone()))
        .expect("the individually valid constraints prepare");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.len() == 1
                && matches!(
                    conflicts[0].conflict(),
                    DirectConstraintConflictKindV1::
                        EqualLengthWithNonUnitRatioAndFixedLength { .. }
                )
                && conflicts[0].constraint_ids()
                    == sorted_ids(&[fixed.id, equal.id, ratio.id])
    ));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids == &sorted_ids(&[fixed.id, equal.id, ratio.id])
    ));

    for removed in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed)
            .map(|(_, record)| record.clone())
            .collect::<Vec<_>>();
        let prepared = prepare(&fixture, &document(subset)).expect("proper subset prepares");
        assert!(
            !matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "removing any cause record must withdraw the rounded-residual proof"
        );
    }
}

#[test]
fn non_reciprocal_ratio_binary64_closure_conflict_is_irredundant() {
    let fixture = Fixture::new();
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 10.0,
    });
    let forward = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let reverse = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[1],
        denominator_edge: fixture.edges[0],
        ratio: 0.25,
    });
    let records = [fixed.clone(), forward.clone(), reverse.clone()];
    let prepared = prepare(&fixture, &document(records.clone()))
        .expect("the individually valid constraints prepare");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.len() == 1
                && matches!(
                    conflicts[0].conflict(),
                    DirectConstraintConflictKindV1::
                        NonReciprocalLengthRatiosWithFixedLength { .. }
                )
                && conflicts[0].constraint_ids()
                    == sorted_ids(&[fixed.id, forward.id, reverse.id])
    ));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids == &sorted_ids(&[fixed.id, forward.id, reverse.id])
    ));

    for removed in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed)
            .map(|(_, record)| record.clone())
            .collect::<Vec<_>>();
        let prepared = prepare(&fixture, &document(subset)).expect("proper subset prepares");
        assert!(
            !matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "removing any cause record must withdraw the binary64 closure proof"
        );
    }
    let reciprocal = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[1],
        denominator_edge: fixture.edges[0],
        ratio: 0.5,
    });
    let prepared = prepare(&fixture, &document([fixed, forward, reciprocal]))
        .expect("reciprocal ratios prepare");
    assert!(
        !matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ),
        "exact reciprocal ratios must not be reported as contradictory"
    );
}

#[test]
fn shared_fixed_length_groups_keep_scan_and_conflict_output_linear() {
    const SHARED_FIXED_COUNT: usize = 1_000;
    const PAIR_COUNT: usize = 1_000;

    let center = VertexId::new();
    let common_end = VertexId::new();
    let mut vertices = vec![
        Vertex {
            id: center,
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: common_end,
            position: Point2::new(1.0, 0.0),
        },
    ];
    let common_edge = EdgeId::new();
    let mut edges = vec![Edge {
        id: common_edge,
        start: center,
        end: common_end,
        kind: EdgeKind::Auxiliary,
    }];
    let mut secondary_edges = Vec::with_capacity(PAIR_COUNT);
    for index in 0..PAIR_COUNT {
        let endpoint = VertexId::new();
        vertices.push(Vertex {
            id: endpoint,
            position: Point2::new(index as f64 + 2.0, 1.0),
        });
        let edge = EdgeId::new();
        edges.push(Edge {
            id: edge,
            start: center,
            end: endpoint,
            kind: EdgeKind::Auxiliary,
        });
        secondary_edges.push(edge);
    }
    let pattern = CreasePattern { vertices, edges };

    let mut records = Vec::with_capacity(SHARED_FIXED_COUNT + 2 * PAIR_COUNT);
    records.extend((0..SHARED_FIXED_COUNT).map(|_| {
        record(GeometricConstraintKindV1::FixedLength {
            edge: common_edge,
            length_mm: 1.0,
        })
    }));
    for edge in secondary_edges {
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }));
        records.push(record(GeometricConstraintKindV1::EqualLength {
            first_edge: common_edge,
            second_edge: edge,
        }));
    }
    let record_count = records.len();
    let raw = document(records);
    let limits = GeometricConstraintLimitsV1 {
        max_vertices: pattern.vertices.len(),
        max_edges: pattern.edges.len(),
        max_constraints: record_count,
        max_references: SHARED_FIXED_COUNT + 3 * PAIR_COUNT,
        max_preflight_checks: record_count,
    };
    let prepared = prepare_geometric_constraints_v1(&pattern, &raw, limits)
        .expect("stress input is exactly within every limit");
    begin_fixed_length_summary_visit_count();
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
        panic!("each equal-length relation directly contradicts fixed lengths");
    };
    assert_eq!(
        finish_fixed_length_summary_visit_count(),
        SHARED_FIXED_COUNT + PAIR_COUNT,
        "each fixed-length assignment must be summarized exactly once regardless of how many equal-length pairs reuse its edge"
    );
    assert_eq!(conflicts.len(), PAIR_COUNT);
    assert!(
        conflicts
            .iter()
            .all(|conflict| conflict.constraint_ids().len() == 3)
    );
    assert_eq!(
        conflicts
            .iter()
            .map(|conflict| conflict.constraint_ids().len())
            .sum::<usize>(),
        3 * PAIR_COUNT
    );

    let one_short = prepare_geometric_constraints_v1(
        &pattern,
        &raw,
        GeometricConstraintLimitsV1 {
            max_preflight_checks: record_count - 1,
            ..limits
        },
    )
    .expect("a preflight work limit does not invalidate persistence");
    assert!(matches!(
        one_short.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len() == record_count
    ));
}

#[test]
fn differing_fixed_length_angle_and_ratio_report_all_cause_ids() {
    let fixture = Fixture::new();
    let length_a = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let length_b = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 2.0,
    });
    let angle_a = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 45.0,
    });
    let angle_b = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[1],
        second_edge: fixture.edges[0],
        angle_degrees: 90.0,
    });
    let ratio_a = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 1.0,
    });
    let ratio_b = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let prepared = prepare(
        &fixture,
        &document([
            ratio_b.clone(),
            length_b.clone(),
            angle_a.clone(),
            length_a.clone(),
            ratio_a.clone(),
            angle_b.clone(),
        ]),
    )
    .expect("valid references");
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
        panic!("different direct scalar assignments must conflict");
    };
    assert_eq!(conflicts.len(), 2);
    for conflict in &conflicts {
        assert!(
            conflict
                .constraint_ids()
                .windows(2)
                .all(|pair| { pair[0].canonical_bytes() < pair[1].canonical_bytes() })
        );
    }
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
        ) && same_ids(conflict.constraint_ids(), &[length_a.id, length_b.id])
    }));
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedAngles { .. }
        ) && same_ids(conflict.constraint_ids(), &[angle_a.id, angle_b.id])
    }));
    assert!(conflicts.iter().all(is_proven_direct_conflict_v1));
}

#[test]
fn horizontal_and_vertical_require_an_exact_noncollapse_witness() {
    let fixture = Fixture::new();
    let horizontal = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let vertical = record(GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[0],
    });
    let prepared = prepare(&fixture, &document([vertical.clone(), horizontal.clone()]))
        .expect("each constraint is locally valid");
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: sorted_ids(&[horizontal.id, vertical.id]),
        }
    );

    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let prepared = prepare(
        &fixture,
        &document([vertical.clone(), fixed.clone(), horizontal.clone()]),
    )
    .expect("positive fixed length excludes the zero-length escape");
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                    edge: fixture.edges[0],
                },
                constraint_ids: sorted_ids(&[horizontal.id, vertical.id, fixed.id]),
            }],
        }
    );
}

#[test]
fn horizontal_and_vertical_use_normalized_edge_constraints_as_noncollapse_witnesses() {
    let fixture = Fixture::new();
    let providers = [
        (
            "point-on-line",
            GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[0],
            },
        ),
        (
            "mirror axis",
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[2],
                second_vertex: fixture.vertices[4],
                axis_edge: fixture.edges[0],
            },
        ),
        (
            "angle-bisector arm",
            GeometricConstraintKindV1::AngleBisector {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                bisector_edge: fixture.edges[2],
            },
        ),
    ];

    for (description, provider_kind) in providers {
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let provider = record(provider_kind);
        let records = vec![vertical.clone(), provider.clone(), horizontal.clone()];
        let prepared = prepare(&fixture, &document(records.clone()))
            .unwrap_or_else(|error| panic!("{description} witness must prepare: {error:?}"));
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: fixture.edges[0],
                    },
                    constraint_ids: sorted_ids(&[horizontal.id, vertical.id, provider.id,]),
                }],
            },
            "{description}"
        );

        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("{description} must feed the bounded direct-conflict oracle")
        };
        assert_eq!(
            constraint_ids,
            sorted_ids(&[horizontal.id, vertical.id, provider.id]),
            "{description}"
        );
        assert_eq!(oracle_calls, 7, "{description}");

        for removed in [horizontal.id, vertical.id, provider.id] {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                !matches!(
                    prepare(&fixture, &document(subset))
                        .expect("proper normalized-edge witness subset")
                        .preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ),
                "{description}: deleting {removed:?} must remove the direct contradiction"
            );
        }
    }
}

#[test]
fn horizontal_and_vertical_detect_every_angle_bisector_edge_role() {
    let fixture = Fixture::new();
    let roles = [
        (fixture.edges[0], fixture.edges[1], fixture.edges[2]),
        (fixture.edges[1], fixture.edges[0], fixture.edges[2]),
        (fixture.edges[1], fixture.edges[2], fixture.edges[0]),
    ];

    for (first_edge, second_edge, bisector_edge) in roles {
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let bisector = record(GeometricConstraintKindV1::AngleBisector {
            vertex: fixture.vertices[0],
            first_edge,
            second_edge,
            bisector_edge,
        });
        assert_eq!(
            prepare(
                &fixture,
                &document([bisector.clone(), horizontal.clone(), vertical.clone()]),
            )
            .expect("every angle-bisector role is locally valid")
            .preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: fixture.edges[0],
                    },
                    constraint_ids: sorted_ids(&[horizontal.id, vertical.id, bisector.id,]),
                }],
            }
        );
    }
}

#[test]
fn horizontal_and_vertical_select_the_canonical_noncollapse_witness() {
    let fixture = Fixture::new();
    let first_horizontal = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let second_horizontal = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let first_vertical = record(GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[0],
    });
    let second_vertical = record(GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[0],
    });
    let fixed = record(fixed_length(&fixture, 0, 1.0));
    let point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[2],
        line_edge: fixture.edges[0],
    });
    let mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
        first_vertex: fixture.vertices[2],
        second_vertex: fixture.vertices[4],
        axis_edge: fixture.edges[0],
    });
    let bisector = record(GeometricConstraintKindV1::AngleBisector {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        bisector_edge: fixture.edges[2],
    });
    let expected_horizontal = [first_horizontal.id, second_horizontal.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let expected_vertical = [first_vertical.id, second_vertical.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let expected_provider = [fixed.id, point.id, mirror.id, bisector.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                edge: fixture.edges[0],
            },
            constraint_ids: sorted_ids(&[
                expected_horizontal,
                expected_vertical,
                expected_provider,
            ]),
        }],
    };
    let mut records = vec![
        first_horizontal,
        second_vertical,
        fixed,
        point,
        second_horizontal,
        first_vertical,
        mirror,
        bisector,
    ];
    let forward = prepare(&fixture, &document(records.clone()))
        .expect("duplicate canonical witnesses prepare")
        .preflight();
    records.reverse();
    let reverse = prepare(&fixture, &document(records))
        .expect("source-reversed canonical witnesses prepare")
        .preflight();
    assert_eq!(forward, expected);
    assert_eq!(reverse, expected);
}

#[test]
fn horizontal_and_vertical_noncollapse_witness_requires_the_same_exact_edge() {
    let fixture = Fixture::new();
    let providers = [
        GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        },
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[2],
            second_vertex: fixture.vertices[4],
            axis_edge: fixture.edges[4],
        },
        GeometricConstraintKindV1::AngleBisector {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
            bisector_edge: fixture.edges[3],
        },
    ];

    for provider_kind in providers {
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let provider = record(provider_kind);
        assert_eq!(
            prepare(
                &fixture,
                &document([horizontal.clone(), vertical.clone(), provider.clone()]),
            )
            .expect("nonmatching exact edge witness prepares")
            .preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: sorted_ids(&[horizontal.id, vertical.id, provider.id,]),
            }
        );
    }
}

#[test]
fn normalized_edge_witness_precedes_general_parallel_for_horizontal_and_vertical() {
    let fixture = Fixture::new();
    let horizontal = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let vertical = record(GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[0],
    });
    let point = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[2],
        line_edge: fixture.edges[0],
    });
    let parallel = record(GeometricConstraintKindV1::Parallel {
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[4],
    });

    assert_eq!(
        prepare(
            &fixture,
            &document([
                parallel.clone(),
                vertical.clone(),
                point.clone(),
                horizontal.clone(),
            ]),
        )
        .expect("fixed normalized-edge witness with incident parallel")
        .preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                    edge: fixture.edges[0],
                },
                constraint_ids: sorted_ids(&[horizontal.id, vertical.id, point.id]),
            }],
        }
    );
    let without_point_records = vec![parallel.clone(), vertical.clone(), horizontal.clone()];
    assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
        &fixture,
        &without_point_records,
        fixture.edges[0],
    );
}

#[test]
fn direct_three_constraint_relations_are_detected() {
    let fixture = Fixture::new();
    let first_length = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let second_length = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[1],
        length_mm: 2.0,
    });
    let equal = record(GeometricConstraintKindV1::EqualLength {
        first_edge: fixture.edges[1],
        second_edge: fixture.edges[0],
    });
    let parallel = record(GeometricConstraintKindV1::Parallel {
        first_edge: fixture.edges[1],
        second_edge: fixture.edges[0],
    });
    let angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 90.0,
    });
    let prepared = prepare(
        &fixture,
        &document([equal, second_length, parallel, first_length, angle]),
    )
    .expect("locally valid");
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
        panic!("direct relations must conflict");
    };
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths { .. }
    )));
    assert!(conflicts.iter().all(is_proven_direct_conflict_v1));
}

#[test]
fn proven_direct_conflict_oracle_cores_are_canonical_and_irredundant() {
    let fixture = Fixture::new();
    let cases = [
        vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            }),
        ],
        vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: 2.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
        ],
        vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: 2.0,
            }),
        ],
        vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[1],
                denominator_edge: fixture.edges[2],
                ratio: 3.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[2],
                denominator_edge: fixture.edges[0],
                ratio: 0.25,
            }),
        ],
    ];

    for records in cases {
        let prepared = prepare(&fixture, &document(records.clone())).expect("valid cause");
        let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
            panic!("the allowlisted direct witness must prove a conflict");
        };
        assert_eq!(conflicts.len(), 1);
        let cause = &conflicts[0];
        assert_eq!(cause.constraint_ids().len(), records.len());
        assert!(
            cause
                .constraint_ids()
                .windows(2)
                .all(|pair| { pair[0].canonical_bytes() < pair[1].canonical_bytes() })
        );

        for removed in cause.constraint_ids() {
            let subset = records
                .iter()
                .filter(|record| record.id != *removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(!matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper witness subset remains valid input")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
    }
}

#[test]
fn fixed_lengths_and_ratio_share_the_solver_binary64_residual() {
    let minimum = f64::from_bits(1);
    let one_up = 1.0_f64.next_up();
    assert_eq!(length_ratio_residual_binary64_v1(6.0, 2.0, 3.0), 0.0);
    assert_eq!(
        length_ratio_residual_binary64_v1(minimum, one_up, minimum),
        0.0,
        "a real-product mismatch can disappear in the implemented rounded multiplication"
    );
    assert_ne!(length_ratio_residual_binary64_v1(0.3, 3.0, 0.1), 0.0);
    assert_ne!(
        length_ratio_residual_binary64_v1(minimum, 0.5, minimum),
        0.0,
        "underflow to zero cannot satisfy a positive fixed numerator"
    );
    assert!(
        !length_ratio_residual_binary64_v1(f64::MAX, 2.0, f64::MAX).is_finite(),
        "overflow is rejected by the numerical residual boundary"
    );

    let fixture = Fixture::new();
    let ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let prepared = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 6.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: 3.0,
            }),
            ratio.clone(),
        ]),
    )
    .expect("exactly compatible fixed lengths and ratio");
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: vec![ratio.id],
        }
    );

    let rounded_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: one_up,
    });
    let rounded_compatible = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: minimum,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: minimum,
            }),
            rounded_ratio.clone(),
        ]),
    )
    .expect("rounded-compatible fixed lengths and ratio");
    assert_eq!(
        rounded_compatible.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: vec![rounded_ratio.id],
        },
        "zero in the shared residual must never become a direct contradiction"
    );
    assert_bounded_direct_oracle_unknown(&rounded_compatible);

    let numerator = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 0.3,
    });
    let denominator = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[1],
        length_mm: 0.1,
    });
    let incompatible_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 3.0,
    });
    let prepared = prepare(
        &fixture,
        &document([
            numerator.clone(),
            denominator.clone(),
            incompatible_ratio.clone(),
        ]),
    )
    .expect("binary64-incompatible fixed lengths and ratio");
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
            },
            constraint_ids: sorted_ids(&[numerator.id, denominator.id, incompatible_ratio.id]),
        }],
    };
    assert_eq!(prepared.preflight(), expected);
    let BoundedDirectMusV1::ProvenUnsatisfiable {
        constraint_ids,
        oracle_calls,
    } = find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the shared non-zero residual must prove the three-record direct cause")
    };
    assert_eq!(
        constraint_ids,
        sorted_ids(&[numerator.id, denominator.id, incompatible_ratio.id])
    );
    assert_eq!(oracle_calls, 7);

    let records = vec![
        numerator.clone(),
        denominator.clone(),
        incompatible_ratio.clone(),
    ];
    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            !matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper rounded-residual witness subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "deleting {removed:?} must remove the direct contradiction"
        );
    }

    let mut reversed = records;
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, &document(reversed))
            .expect("source-order reversed direct cause")
            .preflight(),
        expected
    );

    for (label, numerator_length, ratio, denominator_length) in [
        ("underflow", minimum, 0.5, minimum),
        ("overflow", f64::MAX, 2.0, f64::MAX),
    ] {
        let prepared = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: numerator_length,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: denominator_length,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio,
                }),
            ]),
        )
        .unwrap_or_else(|error| panic!("{label} boundary must prepare: {error:?}"));
        assert!(
            matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict {
                    ref conflicts
                } if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::
                            LengthRatioWithIncompatibleFixedLengths { .. }
                    )
            ),
            "{label}: the shared residual boundary must prove the contradiction"
        );
    }
}

#[test]
fn different_ratios_need_a_fixed_denominator_and_incompatible_binary64_products() {
    let fixture = Fixture::new();
    let numerator_edge = fixture.edges[0];
    let denominator_edge = fixture.edges[1];
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: denominator_edge,
        length_mm: 1.0,
    });
    let first_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge,
        denominator_edge,
        ratio: 2.0,
    });
    let second_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge,
        denominator_edge,
        ratio: 3.0,
    });
    let records = vec![fixed.clone(), first_ratio.clone(), second_ratio.clone()];
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::DifferentLengthRatios {
                numerator_edge,
                denominator_edge,
            },
            constraint_ids: sorted_ids(&[fixed.id, first_ratio.id, second_ratio.id]),
        }],
    };
    let prepared = prepare(&fixture, &document(records.clone()))
        .expect("two incompatible ratio products and a fixed denominator prepare");
    assert_eq!(prepared.preflight(), expected);
    let BoundedDirectMusV1::ProvenUnsatisfiable {
        constraint_ids,
        oracle_calls,
    } = find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the three-record rounded-product contradiction must feed the bounded oracle")
    };
    assert_eq!(
        constraint_ids,
        sorted_ids(&[fixed.id, first_ratio.id, second_ratio.id])
    );
    assert_eq!(oracle_calls, 7);

    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            !matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper product-conflict subset prepares")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "deleting {removed:?} must remove the three-record proof"
        );
    }

    let mut reversed = records;
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, &document(reversed))
            .expect("source-order reversal prepares")
            .preflight(),
        expected,
        "canonical IDs, not source order, select the witness"
    );

    let without_fixed = prepare(
        &fixture,
        &document([first_ratio.clone(), second_ratio.clone()]),
    )
    .expect("the unsafe two-ratio counterexample prepares");
    assert_eq!(
        without_fixed.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: sorted_ids(&[first_ratio.id, second_ratio.id]),
        }
    );
    assert!(
        last_quarantined_direct_conflicts()
            .iter()
            .all(|candidate| !matches!(
                candidate.conflict(),
                DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
            )),
        "an unsafe two-ID ratio pair must remain unchecked without becoming a candidate"
    );
    assert_bounded_direct_oracle_unknown(&without_fixed);

    let duplicate_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge,
        denominator_edge,
        ratio: 2.0,
    });
    let duplicate_only = prepare(
        &fixture,
        &document([fixed.clone(), first_ratio.clone(), duplicate_ratio.clone()]),
    )
    .expect("bit-identical duplicate ratios prepare");
    assert_eq!(
        duplicate_only.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: sorted_ids(&[first_ratio.id, duplicate_ratio.id]),
        },
        "duplicate values never establish a contradiction"
    );

    let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: denominator_edge,
        length_mm: 1.0,
    });
    let canonical_fixed = [fixed.id, duplicate_fixed.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("two fixed denominator IDs have a minimum");
    let duplicate_fixed_group = prepare(
        &fixture,
        &document([
            duplicate_fixed,
            second_ratio.clone(),
            fixed.clone(),
            first_ratio.clone(),
        ]),
    )
    .expect("a consistent duplicate fixed-denominator group prepares");
    assert_eq!(
        duplicate_fixed_group.preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::DifferentLengthRatios {
                    numerator_edge,
                    denominator_edge,
                },
                constraint_ids: sorted_ids(&[canonical_fixed, first_ratio.id, second_ratio.id,]),
            }],
        },
        "the consistent fixed group must select its canonical-smallest ID"
    );

    let conflicting_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: denominator_edge,
        length_mm: 2.0,
    });
    let inconsistent_denominator = prepare(
        &fixture,
        &document([
            fixed.clone(),
            conflicting_fixed.clone(),
            first_ratio.clone(),
            second_ratio.clone(),
        ]),
    )
    .expect("an inconsistent fixed-denominator group still prepares");
    let ConstraintPreflightV1::DirectConflict { conflicts } = inconsistent_denominator.preflight()
    else {
        panic!("the fixed lengths themselves must conflict")
    };
    assert_eq!(conflicts.len(), 1);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengths { edge }
            if *edge == denominator_edge
    ));
    assert_eq!(
        conflicts[0].constraint_ids(),
        sorted_ids(&[fixed.id, conflicting_fixed.id])
    );
}

#[test]
fn different_ratio_products_cover_underflow_rounding_and_overflow_boundaries() {
    let minimum = f64::from_bits(1);
    let one_up = 1.0_f64.next_up();
    let cases = [
        ("ordinary different products", 1.0, 2.0, 3.0, true),
        ("zero versus subnormal", minimum, 0.5, 1.0, true),
        ("both underflow to zero", minimum, 0.25, 0.5, false),
        ("same rounded subnormal", minimum, 1.0, one_up, false),
        ("finite versus overflow", f64::MAX, 1.0, 2.0, true),
        ("both overflow", f64::MAX, 2.0, 3.0, true),
    ];

    for (label, denominator_length, first_value, second_value, proven) in cases {
        let first_product =
            length_ratio_scaled_denominator_binary64_v1(first_value, denominator_length);
        let second_product =
            length_ratio_scaled_denominator_binary64_v1(second_value, denominator_length);
        assert_eq!(
            proven,
            !first_product.is_finite()
                || !second_product.is_finite()
                || first_product != second_product,
            "{label}: the test matrix must match the authoritative product predicate"
        );

        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: denominator_length,
        });
        let first = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: first_value,
        });
        let second = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: second_value,
        });
        let prepared = prepare(
            &fixture,
            &document([fixed.clone(), first.clone(), second.clone()]),
        )
        .unwrap_or_else(|error| panic!("{label}: valid scalar boundary failed: {error:?}"));
        if proven {
            assert!(
                matches!(
                    prepared.preflight(),
                    ConstraintPreflightV1::DirectConflict {
                        ref conflicts
                    } if conflicts.len() == 1
                        && matches!(
                            conflicts[0].conflict(),
                            DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
                        )
                        && conflicts[0].constraint_ids()
                            == sorted_ids(&[fixed.id, first.id, second.id])
                ),
                "{label}: incompatible products must emit the exact three-ID proof"
            );
        } else {
            assert_eq!(
                prepared.preflight(),
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    unchecked_constraint_ids: sorted_ids(&[first.id, second.id]),
                },
                "{label}: a common rounded numerator must stay solver-required"
            );
            assert_bounded_direct_oracle_unknown(&prepared);
        }
    }
}

#[test]
fn three_ratio_cycle_uses_binary64_closure_instead_of_exact_unit_product() {
    assert_eq!(2.0 * 4.0 * 0.125, 1.0);
    assert_eq!(
        f64::from_bits(1) * f64::from_bits(0x7fe0_0000_0000_0000) * 2_f64.powi(51),
        1.0
    );
    assert_ne!(2.0 * 3.0 * 0.25, 1.0);

    let fixture = Fixture::new();
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let first = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let second = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[1],
        denominator_edge: fixture.edges[2],
        ratio: 3.0,
    });
    let third = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[2],
        denominator_edge: fixture.edges[0],
        ratio: 0.25,
    });
    let prepared = prepare(
        &fixture,
        &document([fixed.clone(), first.clone(), second.clone(), third.clone()]),
    )
    .expect("incompatible directed ratio cycle");
    let expected_ids = sorted_ids(&[fixed.id, first.id, second.id, third.id]);
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.len() == 1
                && matches!(
                    conflicts[0].conflict(),
                    DirectConstraintConflictKindV1::
                        NonUnitLengthRatioCycleWithFixedLength { .. }
                )
                && conflicts[0].constraint_ids() == expected_ids
    ));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids == &expected_ids
    ));

    let without_fixed = prepare(&fixture, &document([first, second, third]))
        .expect("zero-length solution remains admissible")
        .preflight();
    assert!(!matches!(
        without_fixed,
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    let compatible = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[1],
                denominator_edge: fixture.edges[2],
                ratio: 4.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[2],
                denominator_edge: fixture.edges[0],
                ratio: 0.125,
            }),
        ]),
    )
    .expect("exactly reciprocal cycle")
    .preflight();
    assert!(!matches!(
        compatible,
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

#[test]
fn reverse_only_binary64_ratio_graph_is_proven_and_irredundant() {
    let fixture = Fixture::new();
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[4],
            length_mm: 7.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[4],
            denominator_edge: fixture.edges[0],
            ratio: 11.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[2],
            ratio: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[3],
            ratio: 5.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[3],
            denominator_edge: fixture.edges[0],
            ratio: 0.1,
        }),
    ];
    let prepared =
        prepare(&fixture, &document(records.clone())).expect("bounded inconsistent ratio graph");
    let expected_ids = {
        let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        canonicalize_constraint_ids(&mut ids);
        ids
    };
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
        panic!("sound reverse-domain closure must prove the inconsistent graph");
    };
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                fixed_edge,
                ratio_constraint_count: 5,
            } if *fixed_edge == fixture.edges[4]
        ) && conflict.constraint_ids() == expected_ids.as_slice()
    }));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids == &expected_ids
    ));

    let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[4],
        length_mm: 7.0,
    });
    let duplicate_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[4],
        denominator_edge: fixture.edges[0],
        ratio: 11.0,
    });
    let mut duplicated = records.clone();
    duplicated.extend([duplicate_fixed.clone(), duplicate_ratio.clone()]);
    let forward = prepare(&fixture, &document(duplicated.clone()))
        .expect("equal duplicate assignments")
        .preflight();
    duplicated.reverse();
    let reversed = prepare(&fixture, &document(duplicated))
        .expect("source-reordered equal duplicate assignments")
        .preflight();
    assert_eq!(forward, reversed);
    assert!(matches!(
        forward,
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(!matches!(
            prepare(&fixture, &document(subset))
                .expect("proper general witness subset")
                .preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    let disconnected_fixed = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[5],
                length_mm: 7.0,
            }),
            records[2].clone(),
            records[3].clone(),
            records[4].clone(),
            records[5].clone(),
        ]),
    )
    .expect("fixed length disconnected from the inconsistent cycle")
    .preflight();
    assert!(!matches!(
        disconnected_fixed,
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

#[test]
fn general_ratio_graph_uses_reverse_domains_without_reciprocal_substitution() {
    let fixture = Fixture::new();
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let connector_a = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 1.0,
    });
    let forward_a = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[1],
        denominator_edge: fixture.edges[2],
        ratio: 2.0,
    });
    let reverse_a = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[2],
        denominator_edge: fixture.edges[1],
        ratio: 0.25,
    });
    let connector_b = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[3],
        ratio: 1.0,
    });
    let forward_b = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[3],
        denominator_edge: fixture.edges[4],
        ratio: 4.0,
    });
    let reverse_b = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[4],
        denominator_edge: fixture.edges[3],
        ratio: 0.125,
    });
    let records = vec![
        fixed.clone(),
        connector_a.clone(),
        forward_a.clone(),
        reverse_a.clone(),
        connector_b.clone(),
        forward_b.clone(),
        reverse_b.clone(),
    ];
    let prepared = prepare(&fixture, &document(records))
        .expect("two inconsistent ratio cycles connected to one remote fixed edge");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|conflict| {
                matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                        ratio_constraint_count: 3,
                        ..
                    }
                ) && conflict.constraint_ids().len() == 4
            })
    ));
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids.len() == 4
    ));

    let reverse_kind = |record: &GeometricConstraintRecordV1| {
        let GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } = record.constraint
        else {
            panic!("ratio record");
        };
        GeometricConstraintRecordV1 {
            id: record.id,
            constraint: GeometricConstraintKindV1::LengthRatio {
                numerator_edge: denominator_edge,
                denominator_edge: numerator_edge,
                ratio: 1.0 / ratio,
            },
        }
    };
    let oriented_forward = prepare(
        &fixture,
        &document([
            fixed.clone(),
            connector_a.clone(),
            forward_a.clone(),
            reverse_a.clone(),
        ]),
    )
    .expect("remote two-edge cycle")
    .preflight();
    let oriented_reverse = prepare(
        &fixture,
        &document([
            fixed,
            reverse_kind(&connector_a),
            reverse_kind(&forward_a),
            reverse_kind(&reverse_a),
        ]),
    )
    .expect("fully direction-reversed remote two-edge cycle")
    .preflight();
    for outcome in [oriented_forward, oriented_reverse] {
        assert!(matches!(
            outcome,
            ConstraintPreflightV1::DirectConflict { conflicts }
                if conflicts.iter().any(|conflict| matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                        ratio_constraint_count: 3,
                        ..
                    }
                ))
        ));
    }
}

#[test]
fn equal_length_graph_returns_a_canonical_shortest_oracle_proof_core() {
    let fixture = Fixture::new();
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[2],
            length_mm: 2.0,
        }),
    ];
    let ConstraintPreflightV1::DirectConflict { conflicts } =
        prepare(&fixture, &document(records.clone()))
            .expect("different fixed lengths connected by an equal-length path")
            .preflight()
    else {
        panic!("equal-length component must conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent {
            equal_constraint_count: 2,
            ..
        }
    ));
    assert_eq!(conflicts[0].constraint_ids().len(), 4);
    for removed in conflicts[0].constraint_ids() {
        let subset = records
            .iter()
            .filter(|record| record.id != *removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(!matches!(
            prepare(&fixture, &document(subset))
                .expect("proper equal-length witness subset")
                .preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    let same_lengths = prepare(
        &fixture,
        &document([
            records[0].clone(),
            records[1].clone(),
            records[2].clone(),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[2],
                length_mm: 1.0,
            }),
        ]),
    )
    .expect("equal fixed lengths across the component")
    .preflight();
    assert!(!matches!(
        same_lengths,
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    let duplicate_equal = record(GeometricConstraintKindV1::EqualLength {
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
    });
    let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let mut duplicated = records.clone();
    duplicated.extend([duplicate_equal, duplicate_fixed]);
    let forward = prepare(&fixture, &document(duplicated.clone()))
        .expect("equal duplicates")
        .preflight();
    duplicated.reverse();
    let reversed = prepare(&fixture, &document(duplicated))
        .expect("source-reordered equal duplicates")
        .preflight();
    assert_eq!(forward, reversed);

    GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| {
        assert_eq!(limit.replace(Some(MAX_GENERAL_EQUAL_GRAPH_WORK_V1)), None);
    });
    let baseline = prepare(&fixture, &document(records.clone()))
        .expect("baseline work-accounted equal graph")
        .preflight();
    let exact_work = GENERAL_EQUAL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
    GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| {
        assert_eq!(
            limit.replace(Some(exact_work)),
            Some(MAX_GENERAL_EQUAL_GRAPH_WORK_V1)
        );
    });
    assert_eq!(
        prepare(&fixture, &document(records.clone()))
            .expect("exact equal-graph work budget")
            .preflight(),
        baseline
    );
    GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
    let limited = prepare(&fixture, &document(records.clone()))
        .expect("one-short equal-graph work budget")
        .preflight();
    GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
    let mut all_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    canonicalize_constraint_ids(&mut all_ids);
    assert_eq!(
        limited,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: all_ids,
        }
    );
}

#[test]
fn equal_length_graph_diamond_with_three_values_is_source_order_invariant() {
    let fixture = Fixture::new();
    let mut records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[3],
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[4],
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[3],
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[2],
            second_edge: fixture.edges[3],
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[4],
        }),
    ];
    let forward = prepare(&fixture, &document(records.clone()))
        .expect("three-value equal-length diamond")
        .preflight();
    records.reverse();
    let reverse = prepare(&fixture, &document(records))
        .expect("source-reordered equal-length diamond")
        .preflight();
    assert_eq!(forward, reverse);
    let ConstraintPreflightV1::DirectConflict { conflicts } = forward else {
        panic!("diamond must select one deterministic shortest conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids().len(), 4);
}

#[test]
fn equal_length_graph_witness_cap_keeps_searching_for_a_short_pair() {
    let scan = |path_edges: usize, include_short_pair: bool| {
        let node_count = path_edges + 1 + usize::from(include_short_pair) * 3;
        let edges = (0..node_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let mut equal_lengths = BTreeMap::new();
        for index in 0..path_edges {
            equal_lengths.insert(
                EdgePairKey::unordered(edges[index], edges[index + 1]),
                vec![ConstraintId::new()],
            );
        }
        let mut fixed_lengths = BTreeMap::from([
            (
                edges[0].canonical_bytes(),
                ScalarGroupSummary::new(ScalarAssignment {
                    id: ConstraintId::new(),
                    value: 1.0,
                }),
            ),
            (
                edges[path_edges].canonical_bytes(),
                ScalarGroupSummary::new(ScalarAssignment {
                    id: ConstraintId::new(),
                    value: 2.0,
                }),
            ),
        ]);
        if include_short_pair {
            let first = path_edges + 1;
            for offset in 0..2 {
                equal_lengths.insert(
                    EdgePairKey::unordered(edges[first + offset], edges[first + offset + 1]),
                    vec![ConstraintId::new()],
                );
            }
            fixed_lengths.insert(
                edges[first].canonical_bytes(),
                ScalarGroupSummary::new(ScalarAssignment {
                    id: ConstraintId::new(),
                    value: 3.0,
                }),
            );
            fixed_lengths.insert(
                edges[first + 2].canonical_bytes(),
                ScalarGroupSummary::new(ScalarAssignment {
                    id: ConstraintId::new(),
                    value: 4.0,
                }),
            );
        }
        let edge_ids = edges
            .iter()
            .map(|edge| (edge.canonical_bytes(), *edge))
            .collect::<BTreeMap<_, _>>();
        general_equal_length_graph_conflict_v1(&equal_lengths, &fixed_lengths, &edge_ids)
    };
    assert_eq!(
        scan(254, false).unwrap().unwrap().constraint_ids().len(),
        256
    );
    assert_eq!(scan(255, false), Err(()));
    assert_eq!(scan(255, true).unwrap().unwrap().constraint_ids().len(), 4);
}

#[test]
fn partially_checked_fixed_angle_and_ratio_kinds_return_unknown() {
    let fixture = Fixture::new();

    let fixed_angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 0.0,
    });
    let both_horizontal = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[1],
            }),
            fixed_angle.clone(),
        ]),
    )
    .expect("locally valid fixed-angle fixture");
    assert_eq!(
        both_horizontal.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: vec![fixed_angle.id],
        }
    );

    let forward_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[0],
        denominator_edge: fixture.edges[1],
        ratio: 2.0,
    });
    let reverse_ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[1],
        denominator_edge: fixture.edges[0],
        ratio: 2.0,
    });
    let inverse_pair = prepare(
        &fixture,
        &document([reverse_ratio.clone(), forward_ratio.clone()]),
    )
    .expect("locally valid inverse ratio fixture");
    assert_eq!(
        inverse_pair.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: sorted_ids(&[forward_ratio.id, reverse_ratio.id]),
        }
    );
}

#[test]
fn parallel_horizontal_vertical_cross_relation_is_detected() {
    let fixture = Fixture::new();
    let records = [
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[4],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[4],
        }),
    ];
    let prepared = prepare(&fixture, &document(records)).expect("locally valid");
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations { .. }
            ))
    ));
}

#[test]
fn parallel_graph_detects_perpendicular_orientation_paths_and_same_node_labels() {
    let fixture = Fixture::new();
    let path_records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[2],
        }),
    ];
    let prepared = prepare(&fixture, &document(path_records.clone()))
        .expect("perpendicular orientations connected by a parallel path");
    assert_solver_required(&prepared.preflight());
    assert_bounded_direct_oracle_unknown(&prepared);
    let mut duplicated = path_records.clone();
    duplicated.extend([
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
    ]);
    let forward = prepare(&fixture, &document(duplicated.clone()))
        .expect("duplicate parallel graph labels")
        .preflight();
    duplicated.reverse();
    let reverse = prepare(&fixture, &document(duplicated))
        .expect("source-reordered duplicate parallel graph labels")
        .preflight();
    assert_eq!(forward, reverse);
    assert_solver_required(&forward);
    for removed in path_records.iter().map(|record| record.id) {
        let subset = path_records
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(!matches!(
            prepare(&fixture, &document(subset))
                .expect("proper parallel-path witness subset")
                .preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    let same_node_records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
    ];
    assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
        &fixture,
        &same_node_records,
        fixture.edges[0],
    );

    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| {
        assert_eq!(
            limit.replace(Some(MAX_GENERAL_PARALLEL_GRAPH_WORK_V1)),
            None
        );
    });
    let baseline = prepare(&fixture, &document(path_records.clone()))
        .expect("work-accounted parallel graph")
        .preflight();
    let exact_work = GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work)));
    assert_eq!(
        prepare(&fixture, &document(path_records.clone()))
            .expect("exact parallel work limit")
            .preflight(),
        baseline
    );
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
    let limited = prepare(&fixture, &document(path_records.clone()))
        .expect("one-short parallel work limit")
        .preflight();
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
    let mut all_ids = path_records
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    canonicalize_constraint_ids(&mut all_ids);
    assert_eq!(
        limited,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: all_ids,
        }
    );
}

#[test]
fn parallel_graph_witness_cap_keeps_searching_for_a_short_remote_pair() {
    let scan = |path_edges: usize, include_short_pair: bool| {
        let node_count = path_edges + 1 + usize::from(include_short_pair) * 3;
        let edges = (0..node_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let mut parallels = BTreeMap::new();
        for index in 0..path_edges {
            parallels.insert(
                EdgePairKey::unordered(edges[index], edges[index + 1]),
                vec![ConstraintId::new()],
            );
        }
        let mut horizontal =
            BTreeMap::from([(edges[0].canonical_bytes(), vec![ConstraintId::new()])]);
        let mut vertical = BTreeMap::from([(
            edges[path_edges].canonical_bytes(),
            vec![ConstraintId::new()],
        )]);
        if include_short_pair {
            let first = path_edges + 1;
            for offset in 0..2 {
                parallels.insert(
                    EdgePairKey::unordered(edges[first + offset], edges[first + offset + 1]),
                    vec![ConstraintId::new()],
                );
            }
            horizontal.insert(edges[first].canonical_bytes(), vec![ConstraintId::new()]);
            vertical.insert(
                edges[first + 2].canonical_bytes(),
                vec![ConstraintId::new()],
            );
        }
        let edge_ids = edges
            .iter()
            .map(|edge| (edge.canonical_bytes(), *edge))
            .collect::<BTreeMap<_, _>>();
        general_parallel_graph_conflict_v1(
            &parallels,
            &horizontal,
            &vertical,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &edge_ids,
        )
    };
    assert_eq!(
        scan(254, false).unwrap().unwrap().constraint_ids().len(),
        256
    );
    assert_eq!(scan(255, false), Err(()));
    assert_eq!(scan(255, true).unwrap().unwrap().constraint_ids().len(), 4);
}

#[test]
fn parallel_graph_diamond_selects_the_canonical_minimum_equal_length_path() {
    let fixture = Fixture::new();
    let horizontal = record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    });
    let vertical = record(GeometricConstraintKindV1::Vertical {
        edge: fixture.edges[3],
    });
    let mut parallel_records = (0..4)
        .map(|_| {
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            })
        })
        .collect::<Vec<_>>();
    parallel_records.sort_unstable_by_key(|record| record.id.canonical_bytes());
    let paths = [
        (fixture.edges[0], fixture.edges[1]),
        (fixture.edges[1], fixture.edges[3]),
        (fixture.edges[0], fixture.edges[2]),
        (fixture.edges[2], fixture.edges[3]),
    ];
    for (record, (first_edge, second_edge)) in parallel_records.iter_mut().zip(paths) {
        record.constraint = GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        };
    }
    let mut records = vec![horizontal.clone(), vertical.clone()];
    records.extend(parallel_records.clone());
    let forward = prepare(&fixture, &document(records.clone()))
        .expect("parallel diamond")
        .preflight();
    records.reverse();
    let reverse = prepare(&fixture, &document(records))
        .expect("source-reordered parallel diamond")
        .preflight();
    assert_eq!(forward, reverse);
    assert_solver_required(&forward);
}

#[test]
fn fixed_angle_parallel_graph_requires_a_nonempty_path_and_excludes_zero_and_180() {
    let fixture = Fixture::new();
    let first_parallel = record(GeometricConstraintKindV1::Parallel {
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
    });
    let second_parallel = record(GeometricConstraintKindV1::Parallel {
        first_edge: fixture.edges[1],
        second_edge: fixture.edges[2],
    });
    let angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[2],
        angle_degrees: 90.0,
    });
    let records = vec![
        first_parallel.clone(),
        second_parallel.clone(),
        angle.clone(),
    ];
    let prepared = prepare(&fixture, &document(records.clone()))
        .expect("nonparallel fixed angle inside a parallel component");
    let baseline = prepared.preflight();
    assert_solver_required(&baseline);
    assert_bounded_direct_oracle_unknown(&prepared);
    let mut reversed_angle = angle.clone();
    reversed_angle.constraint = GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[2],
        second_edge: fixture.edges[0],
        angle_degrees: 90.0,
    };
    assert_eq!(
        prepare(
            &fixture,
            &document([
                first_parallel.clone(),
                second_parallel.clone(),
                reversed_angle,
            ]),
        )
        .expect("operand-reversed fixed angle")
        .preflight(),
        baseline
    );
    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect::<Vec<_>>();
        assert!(!matches!(
            prepare(&fixture, &document(subset))
                .expect("proper fixed-angle parallel witness subset")
                .preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    for allowed in [0.0, -0.0, 180.0] {
        let outcome = prepare(
            &fixture,
            &document([
                first_parallel.clone(),
                second_parallel.clone(),
                record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[2],
                    angle_degrees: allowed,
                }),
            ]),
        )
        .expect("allowed parallel fixed angle")
        .preflight();
        assert!(!matches!(
            outcome,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }
    let signed_zero_duplicates = prepare(
        &fixture,
        &document([
            first_parallel.clone(),
            second_parallel.clone(),
            record(GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[2],
                angle_degrees: 0.0,
            }),
            record(GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[2],
                angle_degrees: -0.0,
            }),
        ]),
    )
    .expect("signed zero fixed-angle duplicates")
    .preflight();
    assert!(!matches!(
        signed_zero_duplicates,
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    for incompatible in [f64::from_bits(1), f64::from_bits(180.0_f64.to_bits() - 1)] {
        let prepared = prepare(
            &fixture,
            &document([
                first_parallel.clone(),
                second_parallel.clone(),
                record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[2],
                    angle_degrees: incompatible,
                }),
            ]),
        )
        .expect("one-ULP incompatible angle");
        assert_solver_required(&prepared.preflight());
    }

    let no_path = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[3],
                second_edge: fixture.edges[4],
            }),
            angle,
        ]),
    )
    .expect("fixed-angle operands do not participate in the parallel component")
    .preflight();
    assert!(!matches!(
        no_path,
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

#[test]
fn fixed_angle_parallel_graph_has_canonical_path_cap_and_work_boundary() {
    let scan = |path_edges: usize| {
        let edges = (0..=path_edges).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let vertex = VertexId::new();
        let angle_id = ConstraintId::new();
        let mut parallels = BTreeMap::new();
        for index in 0..path_edges {
            parallels.insert(
                EdgePairKey::unordered(edges[index], edges[index + 1]),
                vec![ConstraintId::new()],
            );
        }
        let fixed_angles = BTreeMap::from([(
            AngleKey {
                vertex: vertex.canonical_bytes(),
                edges: EdgePairKey::unordered(edges[0], edges[path_edges]),
            },
            vec![ScalarAssignment {
                id: angle_id,
                value: 90.0,
            }],
        )]);
        let vertex_ids = BTreeMap::from([(vertex.canonical_bytes(), vertex)]);
        let edge_ids = edges
            .iter()
            .map(|edge| (edge.canonical_bytes(), *edge))
            .collect::<BTreeMap<_, _>>();
        let result = general_parallel_graph_conflict_v1(
            &parallels,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &fixed_angles,
            &vertex_ids,
            &edge_ids,
        );
        (result, angle_id)
    };

    let (bounded, _) = scan(255);
    assert_eq!(bounded.unwrap().unwrap().constraint_ids().len(), 256);
    assert_eq!(scan(256).0, Err(()));

    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
    let (baseline, _) = scan(3);
    assert!(baseline.is_ok());
    let exact_work = GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work)));
    assert!(scan(3).0.is_ok());
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
    assert_eq!(scan(3).0, Err(()));
    GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
}

#[test]
fn fixed_angle_parallel_diamond_uses_minimum_constraint_ids() {
    let fixture = Fixture::new();
    let angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[3],
        angle_degrees: 90.0,
    });
    let mut parallels = (0..4)
        .map(|_| {
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            })
        })
        .collect::<Vec<_>>();
    parallels.sort_unstable_by_key(|record| record.id.canonical_bytes());
    for (record, (first_edge, second_edge)) in parallels.iter_mut().zip([
        (fixture.edges[0], fixture.edges[2]),
        (fixture.edges[2], fixture.edges[3]),
        (fixture.edges[0], fixture.edges[1]),
        (fixture.edges[1], fixture.edges[3]),
    ]) {
        record.constraint = GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        };
    }
    let mut records = vec![angle.clone()];
    records.extend(parallels.clone());
    let forward = prepare(&fixture, &document(records.clone()))
        .expect("fixed-angle parallel diamond")
        .preflight();
    records.reverse();
    let reverse = prepare(&fixture, &document(records))
        .expect("reordered fixed-angle parallel diamond")
        .preflight();
    assert_eq!(forward, reverse);
    assert_solver_required(&forward);
}

pub(super) fn sorted_ids(ids: &[ConstraintId]) -> Vec<ConstraintId> {
    let mut result = ids.to_vec();
    canonicalize_constraint_ids(&mut result);
    result
}

fn same_ids(actual: &[ConstraintId], expected: &[ConstraintId]) -> bool {
    actual == sorted_ids(expected)
}

pub(super) fn uuid_string<T: Serialize>(id: T) -> String {
    serde_json::to_string(&id)
        .expect("serialize UUID-backed ID")
        .trim_matches('"')
        .to_owned()
}

pub(super) fn deterministic_shuffle<T>(items: &mut [T], state: &mut u64) {
    for index in (1..items.len()).rev() {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let target = (*state as usize) % (index + 1);
        items.swap(index, target);
    }
}

pub(super) fn reverse_unordered_operands(constraint: &mut GeometricConstraintKindV1) {
    match constraint {
        GeometricConstraintKindV1::FixedAngle {
            first_edge,
            second_edge,
            ..
        }
        | GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::AngleBisector {
            first_edge,
            second_edge,
            ..
        } => std::mem::swap(first_edge, second_edge),
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            ..
        } => std::mem::swap(first_vertex, second_vertex),
        GeometricConstraintKindV1::FixedLength { .. }
        | GeometricConstraintKindV1::Horizontal { .. }
        | GeometricConstraintKindV1::Vertical { .. }
        | GeometricConstraintKindV1::PointOnLine { .. }
        | GeometricConstraintKindV1::RotationalSymmetry { .. }
        | GeometricConstraintKindV1::LengthRatio { .. } => {}
    }
}

#[test]
fn parallel_with_perpendicular_orientations_feeds_the_bounded_direct_oracle() {
    for count in [4, 8, 16] {
        let fixture = Fixture::new();
        let mut records = vec![
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[1],
            }),
        ];
        records.extend((3..count).map(|index| {
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[index % 6],
                second_edge: fixture.edges[(index + 1) % 6],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).unwrap();
        let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
            find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("normalized parallel residual cannot accept perpendicular directions")
        };
        assert_eq!(constraint_ids.len(), 3);
        for removed in &constraint_ids {
            let constraints = prepared
                .constraints
                .iter()
                .filter(|record| constraint_ids.contains(&record.id) && record.id != *removed)
                .cloned()
                .collect();
            let subset = GeometricConstraintSetV1 {
                source_pattern: &fixture.pattern,
                constraints,
                raw_mirror_roles: prepared.raw_mirror_roles.clone(),
                max_preflight_checks: prepared.max_preflight_checks,
            };
            assert!(!matches!(
                subset.preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
    }
}

#[test]
fn equal_length_with_different_positive_fixed_lengths_feeds_the_bounded_direct_oracle() {
    for count in [4, 8, 16] {
        let fixture = Fixture::new();
        let mut records = vec![
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: 2.0,
            }),
        ];
        records.extend((3..count).map(|index| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[index % 6],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).unwrap();
        let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
            find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("equal positive lengths cannot have different fixed values")
        };
        assert_eq!(constraint_ids.len(), 3);
        for removed in &constraint_ids {
            let constraints = prepared
                .constraints
                .iter()
                .filter(|record| constraint_ids.contains(&record.id) && record.id != *removed)
                .cloned()
                .collect();
            let subset = GeometricConstraintSetV1 {
                source_pattern: &fixture.pattern,
                constraints,
                raw_mirror_roles: prepared.raw_mirror_roles.clone(),
                max_preflight_checks: prepared.max_preflight_checks,
            };
            assert!(!matches!(
                subset.preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
    }

    let fixture = Fixture::new();
    let compatible = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: 1.0,
            }),
        ]),
    )
    .unwrap();
    assert!(!matches!(
        compatible.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
}

fn bounded_zero_closure_records(
    fixture: &Fixture,
    use_ratio: bool,
    fixed_length: f64,
    ratio: f64,
) -> Vec<GeometricConstraintRecordV1> {
    let mut records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[4],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[4],
        }),
    ];
    records.push(if use_ratio {
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[4],
            ratio,
        })
    } else {
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[4],
        })
    });
    records.push(record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: fixed_length,
    }));
    records
}

fn only_bounded_zero_closure_conflict(
    preflight: &ConstraintPreflightV1,
) -> &DirectConstraintConflictV1 {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("the bounded zero-length closure must prove a conflict");
    };
    let [conflict] = conflicts.as_slice() else {
        panic!("the fixture must emit exactly one conflict: {conflicts:?}");
    };
    assert!(matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure { .. }
    ));
    conflict
}

fn only_nondegenerate_provider_closure_conflict(
    preflight: &ConstraintPreflightV1,
) -> &DirectConstraintConflictV1 {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("the non-degeneracy terminal closure must prove a conflict");
    };
    let [conflict] = conflicts.as_slice() else {
        panic!("the fixture must emit exactly one conflict: {conflicts:?}");
    };
    assert!(matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider { .. }
    ));
    conflict
}

#[test]
fn bounded_zero_length_closure_crosses_equal_length_and_ratio_without_solver_assumptions() {
    for use_ratio in [false, true] {
        for (fixed_length, ratio) in [(f64::from_bits(1), f64::from_bits(1)), (f64::MAX, f64::MAX)]
        {
            let fixture = Fixture::new();
            let records = bounded_zero_closure_records(&fixture, use_ratio, fixed_length, ratio);
            let expected_ids =
                sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
            let prepared = prepare(&fixture, &document(records)).unwrap();
            let preflight = prepared.preflight();
            let conflict = only_bounded_zero_closure_conflict(&preflight);
            assert_eq!(conflict.constraint_ids(), expected_ids);
            assert!(matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    PositiveFixedLengthInBoundedZeroLengthClosure {
                        fixed_edge,
                        forced_zero_edge,
                        horizontal_constraint_count: 1,
                        vertical_constraint_count: 1,
                        zero_propagation_constraint_count: 1,
                    } if *fixed_edge == fixture.edges[0]
                        && *forced_zero_edge == fixture.edges[4]
            ));
        }
    }
}

#[test]
fn bounded_zero_length_closure_serializes_its_distinct_wire_tag_and_counts() {
    let fixture = Fixture::new();
    let records = bounded_zero_closure_records(&fixture, true, 1.0, 2.0);
    let prepared = prepare(&fixture, &document(records)).unwrap();
    let preflight = prepared.preflight();
    let conflict = only_bounded_zero_closure_conflict(&preflight);
    let value = serde_json::to_value(conflict).expect("serialize bounded zero-length conflict");

    assert_eq!(
        value["conflict"],
        json!({
            "kind": "positive_fixed_length_in_bounded_zero_length_closure",
            "fixed_edge": fixture.edges[0],
            "forced_zero_edge": fixture.edges[4],
            "horizontal_constraint_count": 1,
            "vertical_constraint_count": 1,
            "zero_propagation_constraint_count": 1,
        })
    );
    assert_eq!(conflict_sort_key(conflict.conflict()).0, 21);
    assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 4);
}

#[test]
fn bounded_zero_length_closure_uses_multi_edge_coordinate_equality_paths() {
    let fixture = Fixture::new();
    let bridge = EdgeId::new();
    let mut pattern = fixture.pattern.clone();
    pattern.edges.push(Edge {
        id: bridge,
        start: fixture.vertices[6],
        end: fixture.vertices[1],
        kind: EdgeKind::Auxiliary,
    });
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[4],
        }),
        record(GeometricConstraintKindV1::Horizontal { edge: bridge }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[4],
        }),
        record(GeometricConstraintKindV1::Vertical { edge: bridge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[5],
            length_mm: 1.0,
        }),
    ];
    let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
    let prepared = prepare_geometric_constraints_v1(
        &pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .unwrap();
    let preflight = prepared.preflight();
    let conflict = only_bounded_zero_closure_conflict(&preflight);
    assert_eq!(conflict.constraint_ids(), expected_ids);
    assert!(matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
            fixed_edge,
            forced_zero_edge,
            horizontal_constraint_count: 2,
            vertical_constraint_count: 2,
            zero_propagation_constraint_count: 0,
        } if *fixed_edge == fixture.edges[5] && *forced_zero_edge == fixture.edges[5]
    ));
}

#[test]
fn bounded_zero_length_closure_core_is_canonical_and_cardinality_smallest_for_the_oracle() {
    let fixture = Fixture::new();
    let records = bounded_zero_closure_records(&fixture, true, 1.0, 2.0);
    let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
    let baseline = prepare(&fixture, &document(records.clone())).unwrap();
    let baseline_preflight = baseline.preflight();
    assert_eq!(
        only_bounded_zero_closure_conflict(&baseline_preflight).constraint_ids(),
        expected_ids
    );

    let mut reversed = records.clone();
    reversed.reverse();
    let reversed = prepare(&fixture, &document(reversed)).unwrap();
    assert_eq!(reversed.preflight(), baseline_preflight);

    let BoundedDirectMusV1::ProvenUnsatisfiable {
        constraint_ids,
        oracle_calls,
    } = find_bounded_direct_mus_v1(&baseline)
    else {
        panic!("the bounded oracle must retain the exact-zero proof core");
    };
    assert_eq!(constraint_ids, expected_ids);
    assert!(oracle_calls > 0);
    for removed in &constraint_ids {
        let constraints = baseline
            .constraints
            .iter()
            .filter(|record| record.id != *removed)
            .cloned()
            .collect();
        let subset = GeometricConstraintSetV1 {
            source_pattern: &fixture.pattern,
            constraints,
            raw_mirror_roles: baseline.raw_mirror_roles.clone(),
            max_preflight_checks: baseline.max_preflight_checks,
        };
        assert!(
            !matches!(
                subset.preflight(),
                ConstraintPreflightV1::DirectConflict { conflicts }
                    if conflicts.iter().any(|conflict| matches!(
                        conflict.conflict(),
                        DirectConstraintConflictKindV1::
                            PositiveFixedLengthInBoundedZeroLengthClosure { .. }
                    ))
            ),
            "deletion removes this theorem's proof; this is not a semantic SAT claim"
        );
    }
}

#[test]
fn bounded_zero_length_closure_selects_the_canonical_duplicate_core_across_storage_orders() {
    let fixture = Fixture::new();
    let mut records = Vec::new();
    for constraint in [
        GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[4],
        },
        GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[4],
        },
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[4],
            ratio: 2.0,
        },
        GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        },
    ] {
        records.extend([record(constraint.clone()), record(constraint)]);
    }
    let canonical_minimum_for = |predicate: &dyn Fn(&GeometricConstraintKindV1) -> bool| {
        records
            .iter()
            .filter(|record| predicate(&record.constraint))
            .map(|record| record.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("each duplicated role has a canonical minimum")
    };
    let expected_ids = sorted_ids(&[
        canonical_minimum_for(&|kind| matches!(kind, GeometricConstraintKindV1::Horizontal { .. })),
        canonical_minimum_for(&|kind| matches!(kind, GeometricConstraintKindV1::Vertical { .. })),
        canonical_minimum_for(&|kind| {
            matches!(kind, GeometricConstraintKindV1::LengthRatio { .. })
        }),
        canonical_minimum_for(&|kind| {
            matches!(kind, GeometricConstraintKindV1::FixedLength { .. })
        }),
    ]);

    let forward = prepare(&fixture, &document(records.clone())).unwrap();
    assert_eq!(
        only_bounded_zero_closure_conflict(&forward.preflight()).constraint_ids(),
        expected_ids
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable {
        constraint_ids: forward_core,
        ..
    } = find_bounded_direct_mus_v1(&forward)
    else {
        panic!("the bounded oracle must retain the canonical duplicate proof core");
    };
    assert_eq!(forward_core, expected_ids);

    records.reverse();
    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.edges.reverse();
    let reversed = prepare_geometric_constraints_v1(
        &reversed_pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(reversed.preflight(), forward.preflight());
    assert_eq!(
        find_bounded_direct_mus_v1(&reversed),
        find_bounded_direct_mus_v1(&forward)
    );
}

#[test]
fn bounded_zero_length_closure_uses_each_binary64_proven_provider_terminal() {
    let fixture = Fixture::new();
    let providers = [
        (
            ZeroLengthClosureProviderKindV1::PointOnLine,
            fixture.edges[5],
            GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            },
        ),
        (
            ZeroLengthClosureProviderKindV1::MirrorSymmetryAxis,
            fixture.edges[0],
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[2],
                second_vertex: fixture.vertices[4],
                axis_edge: fixture.edges[0],
            },
        ),
        (
            ZeroLengthClosureProviderKindV1::AngleBisector,
            fixture.edges[0],
            GeometricConstraintKindV1::AngleBisector {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                bisector_edge: fixture.edges[2],
            },
        ),
        (
            ZeroLengthClosureProviderKindV1::Parallel,
            fixture.edges[0],
            GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            },
        ),
        (
            ZeroLengthClosureProviderKindV1::FixedAngle,
            fixture.edges[0],
            GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                angle_degrees: 90.0,
            },
        ),
    ];

    for (provider_kind, provider_edge, provider) in providers {
        let records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[4],
                second_edge: provider_edge,
            }),
            record(provider),
        ];
        let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        let prepared = prepare(&fixture, &document(records.clone())).unwrap();
        let preflight = prepared.preflight();
        let conflict = only_nondegenerate_provider_closure_conflict(&preflight);
        assert_eq!(conflict.constraint_ids(), expected_ids);
        assert!(matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::
                ZeroLengthClosureReachesNondegenerateProvider {
                    provider_kind: actual_kind,
                    provider_edge: actual_edge,
                    forced_zero_edge,
                    horizontal_constraint_count: 1,
                    vertical_constraint_count: 1,
                    zero_propagation_constraint_count: 1,
                } if *actual_kind == provider_kind
                    && *actual_edge == provider_edge
                    && *forced_zero_edge == fixture.edges[4]
        ));

        let mut reversed_records = records;
        reversed_records.reverse();
        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.edges.reverse();
        let reversed = prepare_geometric_constraints_v1(
            &reversed_pattern,
            &document(reversed_records),
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(reversed.preflight(), preflight);
    }
}

#[test]
fn nondegenerate_provider_closure_serializes_its_closed_wire_contract() {
    let fixture = Fixture::new();
    let prepared = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[4],
                second_edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
        ]),
    )
    .unwrap();
    let preflight = prepared.preflight();
    let conflict = only_nondegenerate_provider_closure_conflict(&preflight);
    assert_eq!(
        serde_json::to_value(conflict.conflict()).unwrap(),
        json!({
            "kind": "zero_length_closure_reaches_nondegenerate_provider",
            "provider_kind": "parallel",
            "provider_edge": fixture.edges[0],
            "forced_zero_edge": fixture.edges[4],
            "horizontal_constraint_count": 1,
            "vertical_constraint_count": 1,
            "zero_propagation_constraint_count": 1,
        })
    );
    assert_eq!(conflict.constraint_ids().len(), 4);
    assert_eq!(conflict_sort_key(conflict.conflict()).0, 22);
}

#[test]
fn fixed_angle_zero_terminal_rejects_signed_zero_pi_and_radian_underflow_false_positives() {
    let fixture = Fixture::new();
    for angle_degrees in [0.0, -0.0, 180.0, f64::from_bits(1)] {
        let prepared = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::Vertical {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[4],
                    second_edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    angle_degrees,
                }),
            ]),
        )
        .unwrap();
        assert!(
            !matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict { conflicts }
                    if conflicts.iter().any(|conflict| matches!(
                        conflict.conflict(),
                        DirectConstraintConflictKindV1::
                            ZeroLengthClosureReachesNondegenerateProvider {
                                provider_kind:
                                    ZeroLengthClosureProviderKindV1::FixedAngle,
                                ..
                            }
                    ))
            ),
            "{angle_degrees:?} must retain a collapsed binary64 escape"
        );
    }
}

#[test]
fn parallel_overflow_counterexample_forbids_zero_length_propagation() {
    let first = (1.0e308_f64, 0.0_f64);
    let second = (1.0e308_f64, 1.0e-308_f64);
    let cross = first.0 * second.1 - first.1 * second.0;
    let denominator = first.0.hypot(first.1) * second.0.hypot(second.1);
    let residual = cross / denominator;

    assert!(cross.is_finite());
    assert_ne!(cross, 0.0);
    assert!(denominator.is_infinite());
    assert_eq!(residual, 0.0);
}

#[test]
fn bounded_zero_length_closure_obeys_four_eight_sixteen_seventeen_256_and_257_boundaries() {
    for count in [4, 8, 16, 17, 256] {
        let fixture = Fixture::new();
        let mut records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
        let proof_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        records.extend((records.len()..count).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[2],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).unwrap();
        assert_eq!(
            only_bounded_zero_closure_conflict(&prepared.preflight()).constraint_ids(),
            proof_ids
        );
        if count <= MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
            let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
                find_bounded_direct_mus_v1(&prepared)
            else {
                panic!("{count}: the bounded oracle must prove its core");
            };
            assert_eq!(constraint_ids, proof_ids);
        } else {
            assert_eq!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::Unknown { oracle_calls: 0 }
            );
        }
    }

    let fixture = Fixture::new();
    let mut records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
    records.extend((records.len()..257).map(|_| {
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[2],
        })
    }));
    let prepared = prepare(&fixture, &document(records)).unwrap();
    assert!(matches!(
        prepared.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded,
            ..
        }
    ));
    assert_eq!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::Unknown { oracle_calls: 0 }
    );
}

#[test]
fn bounded_zero_length_closure_resource_and_observer_stops_are_fail_closed() {
    struct StopAt {
        phase: bounded_zero_closure::Phase,
        minimum_work: u64,
        control: bounded_zero_closure::ObserverControl,
        checkpoints: usize,
        stopped: bool,
    }

    impl bounded_zero_closure::Observer for StopAt {
        fn checkpoint(
            &mut self,
            checkpoint: bounded_zero_closure::Checkpoint,
        ) -> bounded_zero_closure::ObserverControl {
            self.checkpoints += 1;
            if checkpoint.phase == self.phase && checkpoint.completed_work >= self.minimum_work {
                self.stopped = true;
                self.control
            } else {
                bounded_zero_closure::ObserverControl::Continue
            }
        }
    }

    fn controlled(
        prepared: &GeometricConstraintSetV1<'_>,
        limits: bounded_zero_closure::Limits,
        observer: &mut impl bounded_zero_closure::Observer,
    ) -> ConstraintPreflightV1 {
        preflight_direct_conflicts_with_zero_closure_controls_v1(prepared, limits, observer)
    }

    let fixture = Fixture::new();
    let records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
    let prepared = prepare(&fixture, &document(records.clone())).unwrap();
    let exact_work =
        bounded_zero_closure::required_work(fixture.pattern.edges.len(), records.len()).unwrap();
    let exact_storage = records.len() * 32;

    let mut noop = bounded_zero_closure::NoopObserver;
    assert!(matches!(
        controlled(
            &prepared,
            bounded_zero_closure::Limits {
                max_work: exact_work - 1,
                ..bounded_zero_closure::Limits::default()
            },
            &mut noop,
        ),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
    ));
    assert!(matches!(
        controlled(
            &prepared,
            bounded_zero_closure::Limits {
                max_work: exact_work,
                ..bounded_zero_closure::Limits::default()
            },
            &mut noop,
        ),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    assert!(matches!(
        controlled(
            &prepared,
            bounded_zero_closure::Limits {
                max_storage_units: exact_storage - 1,
                ..bounded_zero_closure::Limits::default()
            },
            &mut noop,
        ),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
            ..
        }
    ));
    assert!(matches!(
        controlled(
            &prepared,
            bounded_zero_closure::Limits {
                max_storage_units: exact_storage,
                ..bounded_zero_closure::Limits::default()
            },
            &mut noop,
        ),
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    for (phase, minimum_work, control, expected_reason) in [
        (
            bounded_zero_closure::Phase::Start,
            0,
            bounded_zero_closure::ObserverControl::Cancelled,
            GeometricConstraintUnknownReasonV1::Cancelled,
        ),
        (
            bounded_zero_closure::Phase::ProofSearch,
            1,
            bounded_zero_closure::ObserverControl::DeadlineReached,
            GeometricConstraintUnknownReasonV1::DeadlineReached,
        ),
    ] {
        let mut stop = StopAt {
            phase,
            minimum_work,
            control,
            checkpoints: 0,
            stopped: false,
        };
        assert!(matches!(
            controlled(
                &prepared,
                bounded_zero_closure::Limits::default(),
                &mut stop,
            ),
            ConstraintPreflightV1::Unknown {
                reason,
                ..
            } if reason == expected_reason
        ));
        assert!(stop.stopped);
        assert!(stop.checkpoints > 0);
    }

    let mut expanded_pattern = fixture.pattern.clone();
    expanded_pattern.edges.extend((0..140).map(|_| Edge {
        id: EdgeId::new(),
        start: fixture.vertices[0],
        end: fixture.vertices[1],
        kind: EdgeKind::Auxiliary,
    }));
    let expanded = prepare_geometric_constraints_v1(
        &expanded_pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .unwrap();
    let mut mid_scan = StopAt {
        phase: bounded_zero_closure::Phase::SourcePatternScan,
        minimum_work: 128,
        control: bounded_zero_closure::ObserverControl::Cancelled,
        checkpoints: 0,
        stopped: false,
    };
    assert!(matches!(
        controlled(
            &expanded,
            bounded_zero_closure::Limits::default(),
            &mut mid_scan,
        ),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::Cancelled,
            ..
        }
    ));
    assert!(mid_scan.stopped);
}

#[test]
fn bounded_subset_cancellation_and_preflight_work_limits_fail_closed() {
    struct CancelAt {
        completed: usize,
        checkpoints: usize,
    }
    impl BoundedDirectMusObserverV1 for CancelAt {
        fn should_cancel(&mut self, completed_oracle_calls: usize) -> bool {
            self.checkpoints += 1;
            completed_oracle_calls >= self.completed
        }
    }

    let fixture = Fixture::new();
    let records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
    let prepared = prepare(&fixture, &document(records.clone())).unwrap();
    let mut immediate = CancelAt {
        completed: 0,
        checkpoints: 0,
    };
    assert_eq!(
        find_bounded_direct_mus_with_observer_v1(&prepared, &mut immediate),
        BoundedDirectMusV1::Unknown { oracle_calls: 0 }
    );
    assert_eq!(immediate.checkpoints, 1);

    let mut after_two = CancelAt {
        completed: 2,
        checkpoints: 0,
    };
    assert_eq!(
        find_bounded_direct_mus_with_observer_v1(&prepared, &mut after_two),
        BoundedDirectMusV1::Unknown { oracle_calls: 2 }
    );

    let limited = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1 {
            max_preflight_checks: 3,
            ..GeometricConstraintLimitsV1::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
    ));

    let mut oversized = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
    oversized.extend((oversized.len()..17).map(|_| {
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[2],
        })
    }));
    let oversized = prepare(&fixture, &document(oversized)).unwrap();
    let mut resource_precedes_cancel = CancelAt {
        completed: 0,
        checkpoints: 0,
    };
    assert_eq!(
        find_bounded_direct_mus_with_observer_v1(&oversized, &mut resource_precedes_cancel,),
        BoundedDirectMusV1::Unknown { oracle_calls: 0 }
    );
    assert_eq!(resource_precedes_cancel.checkpoints, 0);
}
