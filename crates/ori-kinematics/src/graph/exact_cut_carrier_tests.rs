use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    RationalCoefficientV1, TreeKinematicsLimits,
    transform::{length, scale, subtract},
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

fn topology(faces: &[FaceId], hinges: &[TreeHinge]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: hinges
            .iter()
            .map(|hinge| FaceAdjacency {
                edge: hinge.edge(),
                first: hinge.left_face(),
                second: hinge.right_face(),
                assignment: hinge.assignment(),
            })
            .collect(),
        material_components: Vec::new(),
    }
}

struct ExactCutFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    moving: Vec<EdgeId>,
    side_a: Vec<FaceId>,
    side_b: Vec<FaceId>,
}

fn rebuild_fixture_v1(
    mut faces: Vec<FaceId>,
    mut hinges: Vec<TreeHinge>,
    fixed_face: FaceId,
    moving: Vec<EdgeId>,
    side_a: Vec<FaceId>,
    side_b: Vec<FaceId>,
    reverse_storage: bool,
) -> ExactCutFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    ExactCutFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face,
        moving,
        side_a,
        side_b,
    }
}

fn non_cactus_fixture_v1(
    reverse_every_hinge: bool,
    normalized_representation_variants: bool,
    reverse_storage: bool,
) -> ExactCutFixtureV1 {
    let namespace = ProjectId::new();
    let side_a = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("cut-a:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let side_b = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("cut-b:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let mut faces = side_a.iter().chain(&side_b).copied().collect::<Vec<_>>();
    let fixed_face = side_a[0];
    let mut hinges = Vec::new();
    for (side_name, side, offset) in [("a", &side_a, 0.0), ("b", &side_b, 10.0)] {
        for index in 0..side.len() {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("stationary-{side_name}:{index}").as_bytes(),
            );
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                side[index],
                side[(index + 1) % side.len()],
                Point3::new(offset, index as f64, 0.0).unwrap(),
                Point3::new(offset, index as f64 + 1.0, 0.0).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
            ));
        }
    }
    let mut moving = Vec::new();
    for index in 0..3 {
        let edge = EdgeId::derive_v5(namespace, format!("moving:{index}").as_bytes());
        let mut assignment = FoldAssignment::Mountain;
        let mut left = side_a[index];
        let mut right = side_b[index];
        let mut start = Point3::new((index * 2) as f64, 0.0, 0.0).unwrap();
        let mut end = Point3::new((index * 2 + 1) as f64, 0.0, 0.0).unwrap();
        let mut axis = Point3::new(1.0, 0.0, 0.0).unwrap();
        if normalized_representation_variants && index == 1 {
            std::mem::swap(&mut left, &mut right);
            assignment = FoldAssignment::Valley;
        }
        if normalized_representation_variants && index == 2 {
            std::mem::swap(&mut start, &mut end);
            axis = Point3::new(-1.0, -0.0, -0.0).unwrap();
            assignment = FoldAssignment::Valley;
        }
        hinges.push(TreeHinge::new_for_test(
            edge, assignment, left, right, start, end, axis,
        ));
        moving.push(edge);
    }
    if reverse_every_hinge {
        hinges = hinges
            .into_iter()
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
            .collect();
    }
    rebuild_fixture_v1(
        std::mem::take(&mut faces),
        hinges,
        fixed_face,
        moving,
        side_a,
        side_b,
        reverse_storage,
    )
}

#[derive(Clone, Copy)]
enum ScheduleOverrideV1 {
    None,
    ConstantMoving(EdgeId),
    SampleMatchingDivergentMoving(EdgeId),
    NonzeroStationary,
}

fn schedule_v1(
    fixture: &ExactCutFixtureV1,
    override_mode: ScheduleOverrideV1,
) -> CanonicalCycleScheduleV1 {
    let moving = fixture.moving.iter().copied().collect::<HashSet<_>>();
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
            let constant_moving =
                matches!(override_mode, ScheduleOverrideV1::ConstantMoving(candidate) if candidate == edge);
            let divergent =
                matches!(override_mode, ScheduleOverrideV1::SampleMatchingDivergentMoving(candidate) if candidate == edge);
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: if is_moving && !constant_moving {
                    45.0_f64.to_bits()
                } else if !is_moving
                    && matches!(override_mode, ScheduleOverrideV1::NonzeroStationary)
                {
                    1.0_f64.to_bits()
                } else {
                    (-0.0_f64).to_bits()
                },
                chebyshev_coefficients: if !is_moving || constant_moving {
                    Vec::new()
                } else if divergent {
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

fn replace_hinge_v1(
    fixture: ExactCutFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> ExactCutFixtureV1 {
    let mut replacement = Some(replacement);
    let hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            if hinge.edge() == edge {
                replacement.take().unwrap()(hinge)
            } else {
                hinge.clone()
            }
        })
        .collect::<Vec<_>>();
    rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        fixture.fixed_face,
        fixture.moving,
        fixture.side_a,
        fixture.side_b,
        false,
    )
}

#[test]
fn exact_cut_closes_a_non_cartesian_non_cactus_graph_without_dyadic_fallback() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    assert_eq!(fixture.audit.closure_hinges().len(), 4);
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let certificate = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1,
                schedule_limits: CycleScheduleLimitsV1 {
                    max_hinges: fixture.geometry.hinges().len(),
                    max_degree: 0,
                    max_coefficient_bits: 1,
                    max_work: 0,
                },
            },
        )
        .expect("the exact cut identity must certify the complete schedule directly");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn exact_cut_normalizes_storage_face_assignment_axis_and_carrier_origins() {
    for (reverse_every_hinge, variants, reverse_storage) in [
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let fixture = non_cactus_fixture_v1(reverse_every_hinge, variants, reverse_storage);
        let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
        assert!(exact_cut_carrier_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
        for parameter in [0.0, 0.25, 0.5, 1.0] {
            let angles = schedule.evaluate(parameter).unwrap();
            assert!(
                fixture
                    .geometry
                    .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-9)
                    .is_ok(),
                "reverse_every_hinge={reverse_every_hinge}, variants={variants}, \
                 reverse_storage={reverse_storage}, parameter={parameter}"
            );
        }
    }
}

#[test]
fn exact_cut_rejects_partial_and_three_sample_matching_profiles() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    let partial = schedule_v1(
        &fixture,
        ScheduleOverrideV1::ConstantMoving(fixture.moving[2]),
    );
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &partial,
        1.0e-9,
    ));

    let divergent = schedule_v1(
        &fixture,
        ScheduleOverrideV1::SampleMatchingDivergentMoving(fixture.moving[1]),
    );
    assert!(divergent.collective_profile_edges_v1().is_none());
    let reference = fixture.moving[0];
    let changed = fixture.moving[1];
    for parameter in [0.0, 0.5, 1.0] {
        let angles = divergent.evaluate(parameter).unwrap();
        let bits = |edge| {
            angles
                .as_slice()
                .iter()
                .find(|angle| angle.edge() == edge)
                .unwrap()
                .angle_degrees()
                .to_bits()
        };
        assert_eq!(bits(reference), bits(changed));
    }
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &divergent,
        1.0e-9,
    ));
}

#[test]
fn exact_cut_requires_exact_zero_stationary_profiles() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::NonzeroStationary);
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn exact_cut_rejects_a_moving_edge_whose_endpoints_are_in_one_component() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    let changed = fixture.moving[2];
    let internal_face = fixture.side_a[0];
    let fixture = replace_hinge_v1(fixture, changed, move |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            internal_face,
            hinge.start(),
            hinge.end(),
            hinge.axis(),
        )
    });
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn exact_cut_rejects_parallel_offset_nonparallel_and_assignment_mismatch() {
    for mutation in 0..3 {
        let fixture = non_cactus_fixture_v1(false, false, false);
        let changed = fixture.moving[1];
        let fixture = replace_hinge_v1(fixture, changed, |hinge| match mutation {
            0 => {
                let shifted_y = f64::from_bits(1);
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    Point3::new(hinge.start().x(), shifted_y, 0.0).unwrap(),
                    Point3::new(hinge.end().x(), shifted_y, 0.0).unwrap(),
                    hinge.axis(),
                )
            }
            1 => TreeHinge::new_for_test(
                hinge.edge(),
                hinge.assignment(),
                hinge.left_face(),
                hinge.right_face(),
                Point3::new(0.0, 0.0, 0.0).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
            ),
            _ => TreeHinge::new_for_test(
                hinge.edge(),
                FoldAssignment::Valley,
                hinge.left_face(),
                hinge.right_face(),
                hinge.start(),
                hinge.end(),
                hinge.axis(),
            ),
        });
        let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
        assert!(!exact_cut_carrier_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
    }
}

#[test]
fn exact_cut_detects_a_subnormal_offset_without_cross_product_underflow() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    let moving = fixture.moving.iter().copied().collect::<HashSet<_>>();
    let changed = fixture.moving[1];
    let hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            if !moving.contains(&hinge.edge()) {
                return hinge.clone();
            }
            let offset = if hinge.edge() == changed {
                f64::from_bits(1)
            } else {
                0.0
            };
            let parameter = hinge.start().x();
            let start = Point3::new(parameter, parameter, offset).unwrap();
            let end = Point3::new(parameter + 1.0, parameter + 1.0, offset).unwrap();
            let delta = subtract(end, start).unwrap();
            let axis = scale(delta, 1.0 / length(delta).unwrap()).unwrap();
            TreeHinge::new_for_test(
                hinge.edge(),
                hinge.assignment(),
                hinge.left_face(),
                hinge.right_face(),
                start,
                end,
                axis,
            )
        })
        .collect::<Vec<_>>();
    let fixture = rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        fixture.fixed_face,
        fixture.moving,
        fixture.side_a,
        fixture.side_b,
        false,
    );
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn exact_cut_fails_closed_on_degenerate_or_nonfinite_intermediate_axes() {
    for axis in [
        Point3::new(0.0, -0.0, 0.0).unwrap(),
        Point3::new(f64::MAX, 0.0, 0.0).unwrap(),
    ] {
        let fixture = non_cactus_fixture_v1(false, false, false);
        let changed = fixture.moving[0];
        let fixture = replace_hinge_v1(fixture, changed, |hinge| {
            TreeHinge::new_for_test(
                hinge.edge(),
                hinge.assignment(),
                hinge.left_face(),
                hinge.right_face(),
                hinge.start(),
                hinge.end(),
                axis,
            )
        });
        let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
        assert!(!exact_cut_carrier_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
    }
}

#[test]
fn exact_cut_rejects_self_loops_parallel_duplicates_and_audit_edge_mismatch() {
    let fixture = non_cactus_fixture_v1(false, false, false);
    let moving = fixture.moving.iter().copied().collect::<HashSet<_>>();
    let original_audit = fixture.audit.clone();
    let original_faces = fixture.geometry.face_ids().to_vec();
    let original_hinges = fixture.geometry.hinges().to_vec();

    let mut self_loop = original_hinges.clone();
    let hinge = &self_loop[0];
    self_loop[0] = TreeHinge::new_for_test(
        hinge.edge(),
        hinge.assignment(),
        hinge.left_face(),
        hinge.left_face(),
        hinge.start(),
        hinge.end(),
        hinge.axis(),
    );
    assert!(
        exact_cut_components_v1(
            &MaterialHingeGraphGeometry::new_for_test(original_faces.clone(), self_loop),
            &original_audit,
            &moving,
        )
        .is_none()
    );

    let mut duplicate = original_hinges.clone();
    let first = duplicate
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving[0])
        .unwrap()
        .clone();
    let index = duplicate
        .iter()
        .position(|hinge| hinge.edge() == fixture.moving[1])
        .unwrap();
    let second = &duplicate[index];
    duplicate[index] = TreeHinge::new_for_test(
        second.edge(),
        second.assignment(),
        first.left_face(),
        first.right_face(),
        second.start(),
        second.end(),
        second.axis(),
    );
    assert!(
        exact_cut_components_v1(
            &MaterialHingeGraphGeometry::new_for_test(original_faces.clone(), duplicate),
            &original_audit,
            &moving,
        )
        .is_none()
    );

    let missing = MaterialHingeGraphGeometry::new_for_test(
        original_faces.clone(),
        original_hinges[..original_hinges.len() - 1].to_vec(),
    );
    assert!(exact_cut_components_v1(&missing, &original_audit, &moving).is_none());

    let mut extra = original_hinges;
    extra.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"foreign-extra-edge"),
        FoldAssignment::Mountain,
        fixture.side_a[0],
        fixture.side_b[3],
        Point3::new(10.0, 0.0, 0.0).unwrap(),
        Point3::new(11.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let extra = MaterialHingeGraphGeometry::new_for_test(original_faces, extra);
    assert!(exact_cut_components_v1(&extra, &original_audit, &moving).is_none());
}

fn native_upper_fixture_v1() -> ExactCutFixtureV1 {
    let namespace = ProjectId::new();
    let side_a = (0..4_999)
        .map(|index| FaceId::derive_v5(namespace, format!("upper-a:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let side_b = (0..5_000)
        .map(|index| FaceId::derive_v5(namespace, format!("upper-b:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let faces = side_a.iter().chain(&side_b).copied().collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(MAX_EXACT_CUT_CARRIER_HINGES_V1);
    for (side_name, side, offset) in [("a", &side_a, 0.0), ("b", &side_b, 1.0)] {
        for index in 0..side.len() - 1 {
            hinges.push(TreeHinge::new_for_test(
                EdgeId::derive_v5(
                    namespace,
                    format!("upper-stationary-{side_name}:{index}").as_bytes(),
                ),
                FoldAssignment::Mountain,
                side[index],
                side[index + 1],
                Point3::new(offset, index as f64, 0.0).unwrap(),
                Point3::new(offset, index as f64 + 1.0, 0.0).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
            ));
        }
    }
    let mut moving = Vec::new();
    for index in 0..3 {
        let edge = EdgeId::derive_v5(namespace, format!("upper-moving:{index}").as_bytes());
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            side_a[index],
            side_b[index],
            Point3::new((index * 2) as f64, 0.0, 0.0).unwrap(),
            Point3::new((index * 2 + 1) as f64, -0.0, 0.0).unwrap(),
            Point3::new(1.0, -0.0, 0.0).unwrap(),
        ));
        moving.push(edge);
    }
    assert_eq!(faces.len(), 9_999);
    assert_eq!(hinges.len(), MAX_EXACT_CUT_CARRIER_HINGES_V1);
    rebuild_fixture_v1(faces, hinges, side_a[0], moving, side_a, side_b, true)
}

#[test]
fn exact_cut_recognizes_the_native_hinge_boundary_and_rejects_one_over() {
    let fixture = native_upper_fixture_v1();
    assert_eq!(fixture.audit.closure_hinges().len(), 2);
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));

    assert!(bounded_exact_cut_carrier_counts_v1(
        MAX_EXACT_CUT_CARRIER_FACES_V1,
        MAX_EXACT_CUT_CARRIER_HINGES_V1,
    ));
    assert!(!bounded_exact_cut_carrier_counts_v1(
        MAX_EXACT_CUT_CARRIER_FACES_V1 + 1,
        MAX_EXACT_CUT_CARRIER_HINGES_V1,
    ));
    assert!(!bounded_exact_cut_carrier_counts_v1(
        MAX_EXACT_CUT_CARRIER_FACES_V1,
        MAX_EXACT_CUT_CARRIER_HINGES_V1 + 1,
    ));

    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"one-over"),
        FoldAssignment::Mountain,
        fixture.side_a[3],
        fixture.side_b[3],
        Point3::new(20.0, 0.0, 0.0).unwrap(),
        Point3::new(21.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let one_over = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        one_over_hinges,
    );
    let moving = fixture.moving.iter().copied().collect::<HashSet<_>>();
    assert!(exact_cut_components_v1(&one_over, &fixture.audit, &moving).is_none());
}
