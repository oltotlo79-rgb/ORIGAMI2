use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3,
    RationalCoefficientV1, TreeKinematicsLimits,
};

fn face(id: FaceId) -> Face {
    Face {
        id,
        key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
        outer: BoundaryWalk {
            half_edges: Vec::new(),
            signed_double_area: 1.0,
        },
        holes: Vec::new(),
        seams: Vec::new(),
        area: 0.5,
    }
}

fn topology(faces: &[FaceId], hinges: &[(EdgeId, FaceId, FaceId)]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: hinges
            .iter()
            .map(|(edge, first, second)| FaceAdjacency {
                edge: *edge,
                first: *first,
                second: *second,
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    }
}
struct DenseGridFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    column_carriers: Vec<Vec<EdgeId>>,
    row_carriers: Vec<Vec<EdgeId>>,
}

fn dense_grid_fixture_v1(columns: usize, rows: usize, reverse_storage: bool) -> DenseGridFixtureV1 {
    assert!(columns >= 2 && rows >= 2);
    let namespace = ProjectId::new();
    let face_id =
        |x: usize, y: usize| FaceId::derive_v5(namespace, format!("dense-face:{x}:{y}").as_bytes());
    let mut faces = (0..rows)
        .flat_map(|y| (0..columns).map(move |x| face_id(x, y)))
        .collect::<Vec<_>>();
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let fixed_face = face_id(0, 0);
    let mut topology_hinges = Vec::new();
    let mut hinges = Vec::new();
    let mut column_carriers = vec![Vec::new(); columns - 1];
    let mut row_carriers = vec![Vec::new(); rows - 1];
    for (x, carrier) in column_carriers.iter_mut().enumerate() {
        for y in 0..rows {
            let edge = EdgeId::derive_v5(namespace, format!("dense-column:{x}:{y}").as_bytes());
            let left = face_id(x, y);
            let right = face_id(x + 1, y);
            let start = Point3::new((x + 1) as f64, 0.0, y as f64).unwrap();
            let end = Point3::new((x + 1) as f64, 0.0, (y + 1) as f64).unwrap();
            let axis = Point3::new(0.0, 0.0, 1.0).unwrap();
            carrier.push(edge);
            topology_hinges.push((edge, left, right));
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                left,
                right,
                start,
                end,
                axis,
            ));
        }
    }
    for (y, carrier) in row_carriers.iter_mut().enumerate() {
        for x in 0..columns {
            let edge = EdgeId::derive_v5(namespace, format!("dense-row:{x}:{y}").as_bytes());
            let left = face_id(x, y);
            let right = face_id(x, y + 1);
            let start = Point3::new(x as f64, 0.0, (y + 1) as f64).unwrap();
            let end = Point3::new((x + 1) as f64, 0.0, (y + 1) as f64).unwrap();
            let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
            carrier.push(edge);
            topology_hinges.push((edge, left, right));
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                left,
                right,
                start,
                end,
                axis,
            ));
        }
    }
    if reverse_storage {
        topology_hinges.reverse();
        hinges.reverse();
    }
    let source = topology(&faces, &topology_hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    DenseGridFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face,
        column_carriers,
        row_carriers,
    }
}

fn dense_grid_schedule_v1(
    fixture: &DenseGridFixtureV1,
    moving: impl IntoIterator<Item = EdgeId>,
) -> CanonicalCycleScheduleV1 {
    dense_grid_schedule_with_overrides_v1(fixture, moving, 0.0, None)
}

fn dense_grid_schedule_with_overrides_v1(
    fixture: &DenseGridFixtureV1,
    moving: impl IntoIterator<Item = EdgeId>,
    stationary_angle_degrees: f64,
    tampered_profile_edge: Option<EdgeId>,
) -> CanonicalCycleScheduleV1 {
    let moving = moving.into_iter().collect::<HashSet<_>>();
    let mut edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let entries = edges
        .into_iter()
        .map(|edge| {
            let is_moving = moving.contains(&edge);
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: if is_moving {
                    45.0_f64.to_bits()
                } else {
                    stationary_angle_degrees.to_bits()
                },
                chebyshev_coefficients: if is_moving {
                    if tampered_profile_edge == Some(edge) {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 44,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ]
                    } else {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 45,
                                denominator: 1,
                            },
                        ]
                    }
                } else {
                    Vec::new()
                },
            }
        })
        .collect::<Vec<_>>();
    let max_work = entries
        .iter()
        .map(|entry| entry.chebyshev_coefficients.len())
        .sum();
    CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 3,
            max_coefficient_bits: 63,
            max_work,
        },
    )
    .unwrap()
}

#[test]
fn dense_grid_dimension_inference_crosses_the_old_nine_boundary_with_hard_caps() {
    let counts = |columns: usize, rows: usize| {
        (
            columns * rows,
            2 * columns * rows - columns - rows,
            (columns - 1) * (rows - 1),
        )
    };
    for (columns, rows) in [
        (2, 2),
        (2, 10),
        (10, 2),
        (3, 9),
        (3, 10),
        (10, 3),
        (7, 17),
        (3, 2_000),
        (2, 3_334),
        (4, 1_429),
    ] {
        let (faces, hinges, closures) = counts(columns, rows);
        assert_eq!(
            bounded_dense_grid_dimensions_v1(faces, hinges, closures),
            Some((columns.min(rows), columns.max(rows)))
        );
    }
    let (faces, hinges, _) = counts(2, 3_334);
    assert_eq!(hinges, MAX_DENSE_GRID_HINGES_V1);
    assert!(faces < MAX_DENSE_GRID_FACES_V1);
    let (_, one_short_hinges, _) = counts(4, 1_429);
    assert_eq!(one_short_hinges, MAX_DENSE_GRID_HINGES_V1 - 1);

    let (faces, hinges, closures) = counts(2, 3_335);
    assert!(hinges > MAX_DENSE_GRID_HINGES_V1);
    assert_eq!(
        bounded_dense_grid_dimensions_v1(faces, hinges, closures),
        None
    );
    assert_eq!(
        bounded_dense_grid_dimensions_v1(MAX_DENSE_GRID_FACES_V1 + 1, MAX_DENSE_GRID_HINGES_V1, 1,),
        None
    );
    assert_eq!(bounded_dense_grid_dimensions_v1(3, 2, 0), None);
    let (faces, hinges, closures) = counts(3, 10);
    assert_eq!(
        bounded_dense_grid_dimensions_v1(faces, hinges + 1, closures),
        None
    );
    assert_eq!(
        bounded_dense_grid_dimensions_v1(faces, hinges, closures + 1),
        None
    );
}

#[test]
fn dense_grid_recognizes_the_native_hinge_limit_with_bounded_work() {
    let fixture = dense_grid_fixture_v1(2, 3_334, true);
    assert_eq!(fixture.geometry.hinges().len(), MAX_DENSE_GRID_HINGES_V1);
    assert_eq!(fixture.geometry.face_ids().len(), 6_668);
    let recognized = recognize_dense_grid_v1(&fixture.geometry, &fixture.audit)
        .expect("the maximum native dense grid must remain recognizable");
    assert_eq!((recognized.columns, recognized.rows), (2, 3_334));
    assert_eq!(recognized.hinges.len(), MAX_DENSE_GRID_HINGES_V1);
}

#[test]
fn dense_grid_two_by_n_and_larger_cases_close_in_both_orientations_and_storage_orders() {
    for (columns, rows) in [(2, 2), (2, 10), (10, 2), (3, 10), (10, 3)] {
        for reverse_storage in [false, true] {
            let fixture = dense_grid_fixture_v1(columns, rows, reverse_storage);
            let moving = fixture
                .column_carriers
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let schedule = dense_grid_schedule_v1(&fixture, moving);
            assert!(
                dense_parallel_grid_cycle_closure_premises_v1(
                    &fixture.geometry,
                    &fixture.audit,
                    fixture.fixed_face,
                    &schedule,
                    1.0e-9,
                ),
                "{columns}x{rows}, reverse={reverse_storage}"
            );
            let certificate = fixture
                .geometry
                .prove_dyadic_schedule_closure_v1(
                    &fixture.audit,
                    fixture.fixed_face,
                    &schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 0,
                        max_leaves: 1,
                        max_work: 1,
                        schedule_limits: CycleScheduleLimitsV1 {
                            max_hinges: fixture.geometry.hinges().len(),
                            max_degree: 1,
                            max_coefficient_bits: 63,
                            max_work: fixture.geometry.hinges().len().saturating_mul(2),
                        },
                    },
                )
                .expect("the authenticated dense-grid identity closes the full schedule");
            assert_eq!(certificate.leaves().len(), 1);
        }
    }
}

#[test]
fn dense_grid_requires_complete_carriers_and_rejects_mixed_families() {
    let fixture = dense_grid_fixture_v1(4, 10, false);
    let one_carrier = fixture.column_carriers[0].clone();
    let schedule = dense_grid_schedule_v1(&fixture, one_carrier.clone());
    assert!(dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));

    let partial = one_carrier[..one_carrier.len() - 1].to_vec();
    let schedule = dense_grid_schedule_v1(&fixture, partial);
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));

    let mut mixed = one_carrier;
    mixed.pop();
    mixed.push(fixture.column_carriers[1][0]);
    let schedule = dense_grid_schedule_v1(&fixture, mixed);
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));

    let mixed_families = fixture.column_carriers[0]
        .iter()
        .chain(&fixture.row_carriers[0])
        .copied()
        .collect::<Vec<_>>();
    let schedule = dense_grid_schedule_v1(&fixture, mixed_families);
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn dense_grid_admits_arbitrary_nonempty_complete_carrier_subsets() {
    let fixture = dense_grid_fixture_v1(6, 7, false);
    let selected = |carriers: &[Vec<EdgeId>], indices: &[usize]| {
        indices
            .iter()
            .flat_map(|index| carriers[*index].iter().copied())
            .collect::<Vec<_>>()
    };
    for moving in [
        selected(&fixture.column_carriers, &[0, 2, 4]),
        selected(&fixture.row_carriers, &[1, 3]),
    ] {
        let schedule = dense_grid_schedule_v1(&fixture, moving);
        assert!(dense_parallel_grid_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
    }
}

#[test]
fn dense_grid_rejects_profiles_that_only_match_at_three_samples() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let moving = fixture.column_carriers[0].clone();
    let tampered_edge = moving[5];
    let reference_edge = moving[0];
    let schedule =
        dense_grid_schedule_with_overrides_v1(&fixture, moving, 0.0, Some(tampered_edge));
    for parameter in [0.0, 0.5, 1.0] {
        let angles = schedule.evaluate(parameter).unwrap();
        let angle = |edge| {
            angles
                .as_slice()
                .iter()
                .find(|angle| angle.edge() == edge)
                .unwrap()
                .angle_degrees()
                .to_bits()
        };
        assert_eq!(angle(reference_edge), angle(tampered_edge));
    }
    let interior = schedule.evaluate(0.75).unwrap();
    let angle = |edge| {
        interior
            .as_slice()
            .iter()
            .find(|angle| angle.edge() == edge)
            .unwrap()
            .angle_degrees()
            .to_bits()
    };
    assert_ne!(angle(reference_edge), angle(tampered_edge));
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn dense_grid_rejects_nonzero_exact_constant_stationary_hinges() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let moving = fixture.column_carriers[0].clone();
    let schedule = dense_grid_schedule_with_overrides_v1(&fixture, moving, 1.0, None);
    assert!(schedule.collective_profile_edges_v1().is_some());
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn dense_grid_recognition_authenticates_every_cartesian_adjacency_slot() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let first_edge = fixture.column_carriers[0][0];
    let second_edge = fixture.column_carriers[1][9];
    let first = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == first_edge)
        .unwrap();
    let second = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == second_edge)
        .unwrap();
    let mut topology_hinges = Vec::new();
    let mut altered_hinges = Vec::new();
    for hinge in fixture.geometry.hinges() {
        let right = if hinge.edge() == first_edge {
            second.right_face()
        } else if hinge.edge() == second_edge {
            first.right_face()
        } else {
            hinge.right_face()
        };
        topology_hinges.push((hinge.edge(), hinge.left_face(), right));
        altered_hinges.push(TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            right,
            hinge.start(),
            hinge.end(),
            hinge.axis(),
        ));
    }
    let source = topology(fixture.geometry.face_ids(), &topology_hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    let altered = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        altered_hinges,
    );
    assert!(
        recognize_dense_grid_v1(&altered, &audit).is_none(),
        "matching aggregate counts must not substitute for a complete Cartesian grid"
    );
}

#[test]
fn dense_grid_axis_and_line_authentication_replay_non_cardinal_binary64_segments() {
    let start = Point3::new(
        f64::from_bits(0x3ff0_0000_0000_0001),
        0.0,
        f64::from_bits(0x3ffb_b67a_e858_4caa),
    )
    .unwrap();
    let end = Point3::new(
        f64::from_bits(0x3ff8_0000_0000_0002),
        0.0,
        f64::from_bits(0x4004_c8dc_2e42_3980),
    )
    .unwrap();
    let delta = subtract(end, start).unwrap();
    let axis = scale(delta, 1.0 / length(delta).unwrap()).unwrap();
    assert_ne!(
        delta.x() * axis.z() - delta.z() * axis.x(),
        0.0,
        "the redundant rounded cross reproduces the former false rejection"
    );

    let left = FaceId::new();
    let right = FaceId::new();
    let hinge = TreeHinge::new_for_test(
        EdgeId::new(),
        FoldAssignment::Mountain,
        left,
        right,
        start,
        end,
        axis,
    );
    assert!(dense_grid_valid_axis_line_v1(DenseGridHingeV1 {
        hinge: &hinge,
        family: DenseGridHingeFamilyV1::ColumnBoundary,
        carrier: 0,
        segment: 0,
        forward_face: left,
    }));

    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let twice_delta = scale(delta, 2.0).unwrap();
    let reference = TreeHinge::new_for_test(
        EdgeId::new(),
        FoldAssignment::Mountain,
        left,
        right,
        origin,
        delta,
        axis,
    );
    let candidate = TreeHinge::new_for_test(
        EdgeId::new(),
        FoldAssignment::Mountain,
        left,
        right,
        delta,
        twice_delta,
        axis,
    );
    let reference = DenseGridHingeV1 {
        hinge: &reference,
        family: DenseGridHingeFamilyV1::ColumnBoundary,
        carrier: 0,
        segment: 0,
        forward_face: left,
    };
    let candidate = DenseGridHingeV1 {
        hinge: &candidate,
        family: DenseGridHingeFamilyV1::ColumnBoundary,
        carrier: 0,
        segment: 1,
        forward_face: left,
    };
    assert!(dense_grid_valid_axis_line_v1(reference));
    assert!(dense_grid_valid_axis_line_v1(candidate));
    assert!(
        dense_grid_same_directed_line_v1(reference, candidate),
        "raw replayed deltas must prove the exact line without reusing the rounded unit axis"
    );
}

#[test]
fn dense_grid_carrier_requires_one_exact_directed_rotation_line() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let moving = fixture.column_carriers[0].clone();
    let changed_edge = moving[5];
    let mut altered_hinges = Vec::new();
    for hinge in fixture.geometry.hinges() {
        altered_hinges.push(TreeHinge::new_for_test(
            hinge.edge(),
            if hinge.edge() == changed_edge {
                FoldAssignment::Valley
            } else {
                hinge.assignment()
            },
            hinge.left_face(),
            hinge.right_face(),
            hinge.start(),
            hinge.end(),
            hinge.axis(),
        ));
    }
    let altered = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        altered_hinges,
    );
    let altered_fixture = DenseGridFixtureV1 {
        geometry: altered,
        audit: fixture.audit,
        fixed_face: fixture.fixed_face,
        column_carriers: fixture.column_carriers,
        row_carriers: fixture.row_carriers,
    };
    let schedule = dense_grid_schedule_v1(&altered_fixture, moving);
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &altered_fixture.geometry,
        &altered_fixture.audit,
        altered_fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn dense_grid_generator_is_invariant_to_reversed_hinge_storage_orientation() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let moving = fixture.column_carriers[0].clone();
    let mut hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let axis = hinge.axis();
            TreeHinge::new_for_test(
                hinge.edge(),
                hinge.assignment(),
                hinge.right_face(),
                hinge.left_face(),
                hinge.end(),
                hinge.start(),
                Point3::new(-axis.x(), -axis.y(), -axis.z()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    hinges.reverse();
    let reversed_fixture = DenseGridFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(
            fixture.geometry.face_ids().to_vec(),
            hinges,
        ),
        audit: fixture.audit,
        fixed_face: fixture.fixed_face,
        column_carriers: fixture.column_carriers,
        row_carriers: fixture.row_carriers,
    };
    let schedule = dense_grid_schedule_v1(&reversed_fixture, moving);
    assert!(dense_parallel_grid_cycle_closure_premises_v1(
        &reversed_fixture.geometry,
        &reversed_fixture.audit,
        reversed_fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn dense_grid_rejects_parallel_carrier_segment_offset_by_one_ulp() {
    let fixture = dense_grid_fixture_v1(3, 10, false);
    let moving = fixture.column_carriers[0].clone();
    let changed_edge = moving[5];
    let hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let mut start = hinge.start();
            let mut end = hinge.end();
            if hinge.edge() == changed_edge {
                let shifted_x = f64::from_bits(start.x().to_bits() + 1);
                start = Point3::new(shifted_x, start.y(), start.z()).unwrap();
                end = Point3::new(shifted_x, end.y(), end.z()).unwrap();
            }
            TreeHinge::new_for_test(
                hinge.edge(),
                hinge.assignment(),
                hinge.left_face(),
                hinge.right_face(),
                start,
                end,
                hinge.axis(),
            )
        })
        .collect::<Vec<_>>();
    let altered_fixture = DenseGridFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(
            fixture.geometry.face_ids().to_vec(),
            hinges,
        ),
        audit: fixture.audit,
        fixed_face: fixture.fixed_face,
        column_carriers: fixture.column_carriers,
        row_carriers: fixture.row_carriers,
    };
    let schedule = dense_grid_schedule_v1(&altered_fixture, moving);
    assert!(!dense_parallel_grid_cycle_closure_premises_v1(
        &altered_fixture.geometry,
        &altered_fixture.audit,
        altered_fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}
