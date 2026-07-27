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

struct BridgeMotionFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    bridges: Vec<EdgeId>,
    cycle_edges: Vec<EdgeId>,
    core_a: Vec<FaceId>,
    core_b: Vec<FaceId>,
}

fn rebuild_fixture_v1(
    mut faces: Vec<FaceId>,
    mut hinges: Vec<TreeHinge>,
    bridges: Vec<EdgeId>,
    cycle_edges: Vec<EdgeId>,
    core_a: Vec<FaceId>,
    core_b: Vec<FaceId>,
    reverse_storage: bool,
) -> BridgeMotionFixtureV1 {
    let fixed_face = core_a[0];
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    BridgeMotionFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face,
        bridges,
        cycle_edges,
        core_a,
        core_b,
    }
}

fn append_complete_core_v1(
    namespace: ProjectId,
    name: &str,
    faces: &[FaceId],
    hinges: &mut Vec<TreeHinge>,
    cycle_edges: &mut Vec<EdgeId>,
    coordinate_offset: f64,
) {
    for first in 0..faces.len() {
        for second in first + 1..faces.len() {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("{name}-cycle:{first}:{second}").as_bytes(),
            );
            let coordinate = coordinate_offset + cycle_edges.len() as f64 * 2.0;
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                faces[first],
                faces[second],
                Point3::new(coordinate, 10.0, 0.0).unwrap(),
                Point3::new(coordinate + 1.0, 10.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ));
            cycle_edges.push(edge);
        }
    }
}

fn bridge_motion_fixture_v1(
    reverse_every_hinge: bool,
    reverse_storage: bool,
) -> BridgeMotionFixtureV1 {
    let namespace = ProjectId::new();
    let core_a = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("bridge-a:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let core_b = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("bridge-b:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let core_c = (0..3)
        .map(|index| FaceId::derive_v5(namespace, format!("bridge-c:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let faces = core_a
        .iter()
        .chain(&core_b)
        .chain(&core_c)
        .copied()
        .collect::<Vec<_>>();
    let mut hinges = Vec::new();
    let mut cycle_edges = Vec::new();
    append_complete_core_v1(namespace, "a", &core_a, &mut hinges, &mut cycle_edges, 0.0);
    append_complete_core_v1(
        namespace,
        "b",
        &core_b,
        &mut hinges,
        &mut cycle_edges,
        100.0,
    );
    append_complete_core_v1(
        namespace,
        "c",
        &core_c,
        &mut hinges,
        &mut cycle_edges,
        200.0,
    );

    let first_bridge = EdgeId::derive_v5(namespace, b"bridge-motion:first");
    let second_bridge = EdgeId::derive_v5(namespace, b"bridge-motion:second");
    hinges.push(TreeHinge::new_for_test(
        first_bridge,
        FoldAssignment::Mountain,
        core_a[0],
        core_b[0],
        Point3::new(0.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    hinges.push(TreeHinge::new_for_test(
        second_bridge,
        FoldAssignment::Valley,
        core_b[1],
        core_c[0],
        Point3::new(2.0, 0.0, 0.0).unwrap(),
        Point3::new(2.0, 1.0, 0.0).unwrap(),
        Point3::new(0.0, 1.0, 0.0).unwrap(),
    ));
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
        faces,
        hinges,
        vec![first_bridge, second_bridge],
        cycle_edges,
        core_a,
        core_b,
        reverse_storage,
    )
}

#[derive(Clone, Copy)]
enum ScheduleOverrideV1 {
    None,
    ActiveCycle(EdgeId),
    NonzeroConstantCycle(EdgeId),
}

fn schedule_v1(
    fixture: &BridgeMotionFixtureV1,
    override_mode: ScheduleOverrideV1,
) -> CanonicalCycleScheduleV1 {
    let bridge_set = fixture.bridges.iter().copied().collect::<HashSet<_>>();
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
            let first_bridge = fixture.bridges.first().copied() == Some(edge);
            let bridge = bridge_set.contains(&edge);
            let active_cycle =
                matches!(override_mode, ScheduleOverrideV1::ActiveCycle(candidate) if candidate == edge);
            let nonzero_cycle =
                matches!(override_mode, ScheduleOverrideV1::NonzeroConstantCycle(candidate) if candidate == edge);
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: if first_bridge {
                    45.0_f64.to_bits()
                } else if bridge {
                    60.0_f64.to_bits()
                } else if active_cycle {
                    45.0_f64.to_bits()
                } else if nonzero_cycle {
                    1.0_f64.to_bits()
                } else {
                    (-0.0_f64).to_bits()
                },
                chebyshev_coefficients: if first_bridge {
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 30,
                            denominator: 1,
                        },
                    ]
                } else if bridge {
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 15,
                            denominator: 1,
                        },
                    ]
                } else if active_cycle {
                    vec![
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
            max_degree: 1,
            max_coefficient_bits: 63,
            max_work,
        },
    )
    .unwrap()
}

fn replace_hinge_v1(
    fixture: BridgeMotionFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> BridgeMotionFixtureV1 {
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
        fixture.bridges,
        fixture.cycle_edges,
        fixture.core_a,
        fixture.core_b,
        false,
    )
}

#[test]
fn bridge_motion_certifies_non_cactus_cores_with_independent_noncommuting_profiles() {
    let fixture = bridge_motion_fixture_v1(false, false);
    assert_eq!(fixture.audit.closure_hinges().len(), 7);
    let recognized = recognize_bridge_edges_v1(&fixture.geometry, &fixture.audit).unwrap();
    let recognized_edges = fixture
        .geometry
        .hinges()
        .iter()
        .zip(recognized)
        .filter_map(|(hinge, bridge)| bridge.then_some(hinge.edge()))
        .collect::<HashSet<_>>();
    assert_eq!(recognized_edges, fixture.bridges.iter().copied().collect());

    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(bridge_only_motion_cycle_closure_premises_v1(
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
                    max_hinges: 0,
                    max_degree: 0,
                    max_coefficient_bits: 1,
                    max_work: 0,
                },
            },
        )
        .expect("bridge-only motion must bypass interval fallback");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn bridge_motion_is_invariant_to_hinge_orientation_and_storage_permutation() {
    for (reverse_every_hinge, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = bridge_motion_fixture_v1(reverse_every_hinge, reverse_storage);
        let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
        assert!(bridge_only_motion_cycle_closure_premises_v1(
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
                "reverse_every_hinge={reverse_every_hinge}, reverse_storage={reverse_storage}, \
                 parameter={parameter}"
            );
        }
    }
}

#[test]
fn bridge_motion_rejects_every_nonzero_or_nonconstant_cycle_edge() {
    let fixture = bridge_motion_fixture_v1(false, false);
    for override_mode in [
        ScheduleOverrideV1::ActiveCycle(fixture.cycle_edges[0]),
        ScheduleOverrideV1::NonzeroConstantCycle(fixture.cycle_edges[0]),
    ] {
        let schedule = schedule_v1(&fixture, override_mode);
        assert!(!bridge_only_motion_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
    }
}

#[test]
fn bridge_motion_rejects_an_active_edge_after_a_second_path_makes_it_cyclic() {
    let fixture = bridge_motion_fixture_v1(false, false);
    let namespace = ProjectId::new();
    let extra_edge = EdgeId::derive_v5(namespace, b"second-core-path");
    let mut hinges = fixture.geometry.hinges().to_vec();
    hinges.push(TreeHinge::new_for_test(
        extra_edge,
        FoldAssignment::Mountain,
        fixture.core_a[2],
        fixture.core_b[2],
        Point3::new(300.0, 0.0, 0.0).unwrap(),
        Point3::new(301.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let mut cycle_edges = fixture.cycle_edges;
    cycle_edges.push(extra_edge);
    let fixture = rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        fixture.bridges,
        cycle_edges,
        fixture.core_a,
        fixture.core_b,
        false,
    );
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(!bridge_only_motion_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));
}

#[test]
fn bridge_motion_rejects_degenerate_and_overflowing_active_bridge_axes() {
    for mutation in 0..2 {
        let fixture = bridge_motion_fixture_v1(false, false);
        let changed = fixture.bridges[0];
        let fixture = replace_hinge_v1(fixture, changed, |hinge| {
            if mutation == 0 {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    hinge.start(),
                    hinge.end(),
                    Point3::new(0.0, -0.0, 0.0).unwrap(),
                )
            } else {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    Point3::new(-f64::MAX, 0.0, 0.0).unwrap(),
                    Point3::new(f64::MAX, 0.0, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                )
            }
        });
        let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
        assert!(!bridge_only_motion_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
    }
}

#[test]
fn bridge_recognition_rejects_self_loops_parallel_pairs_and_audit_mismatch() {
    let fixture = bridge_motion_fixture_v1(false, false);
    let faces = fixture.geometry.face_ids().to_vec();
    let hinges = fixture.geometry.hinges().to_vec();
    let audit = fixture.audit.clone();

    let mut self_loop = hinges.clone();
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
        recognize_bridge_edges_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces.clone(), self_loop),
            &audit,
        )
        .is_none()
    );

    let mut duplicate = hinges.clone();
    let first = duplicate[0].clone();
    let second = &duplicate[1];
    duplicate[1] = TreeHinge::new_for_test(
        second.edge(),
        second.assignment(),
        first.left_face(),
        first.right_face(),
        second.start(),
        second.end(),
        second.axis(),
    );
    assert!(
        recognize_bridge_edges_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces.clone(), duplicate),
            &audit,
        )
        .is_none()
    );

    let missing = MaterialHingeGraphGeometry::new_for_test(
        faces.clone(),
        hinges[..hinges.len() - 1].to_vec(),
    );
    assert!(recognize_bridge_edges_v1(&missing, &audit).is_none());

    let mut extra = hinges;
    extra.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"foreign-extra"),
        FoldAssignment::Mountain,
        fixture.core_a[1],
        fixture.core_b[3],
        Point3::new(400.0, 0.0, 0.0).unwrap(),
        Point3::new(401.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let extra = MaterialHingeGraphGeometry::new_for_test(faces, extra);
    assert!(recognize_bridge_edges_v1(&extra, &audit).is_none());
}

fn native_upper_fixture_v1() -> BridgeMotionFixtureV1 {
    let namespace = ProjectId::new();
    let core_a = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("upper-core:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let mut faces = core_a.clone();
    let mut hinges = Vec::with_capacity(MAX_BRIDGE_MOTION_HINGES_V1);
    let mut cycle_edges = Vec::new();
    append_complete_core_v1(
        namespace,
        "upper",
        &core_a,
        &mut hinges,
        &mut cycle_edges,
        0.0,
    );
    let mut bridges = Vec::with_capacity(MAX_BRIDGE_MOTION_HINGES_V1 - hinges.len());
    let mut previous = core_a[0];
    for index in 0..MAX_BRIDGE_MOTION_HINGES_V1 - hinges.len() {
        let face = FaceId::derive_v5(namespace, format!("upper-chain:{index}").as_bytes());
        let edge = EdgeId::derive_v5(namespace, format!("upper-bridge:{index}").as_bytes());
        faces.push(face);
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            previous,
            face,
            Point3::new(index as f64, 0.0, 0.0).unwrap(),
            Point3::new(index as f64 + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ));
        bridges.push(edge);
        previous = face;
    }
    assert_eq!(faces.len(), 9_998);
    assert_eq!(hinges.len(), MAX_BRIDGE_MOTION_HINGES_V1);
    rebuild_fixture_v1(
        faces,
        hinges,
        bridges,
        cycle_edges,
        core_a.clone(),
        core_a,
        true,
    )
}

#[test]
fn bridge_motion_accepts_the_native_hinge_boundary_and_rejects_one_over() {
    let fixture = native_upper_fixture_v1();
    assert_eq!(fixture.audit.closure_hinges().len(), 3);
    let recognized = recognize_bridge_edges_v1(&fixture.geometry, &fixture.audit).unwrap();
    assert_eq!(
        recognized
            .into_iter()
            .filter(|is_bridge| *is_bridge)
            .count(),
        fixture.bridges.len()
    );
    let schedule = schedule_v1(&fixture, ScheduleOverrideV1::None);
    assert!(bridge_only_motion_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        1.0e-9,
    ));

    assert!(bounded_bridge_motion_counts_v1(
        MAX_BRIDGE_MOTION_FACES_V1,
        MAX_BRIDGE_MOTION_HINGES_V1,
    ));
    assert!(!bounded_bridge_motion_counts_v1(
        MAX_BRIDGE_MOTION_FACES_V1 + 1,
        MAX_BRIDGE_MOTION_HINGES_V1,
    ));
    assert!(!bounded_bridge_motion_counts_v1(
        MAX_BRIDGE_MOTION_FACES_V1,
        MAX_BRIDGE_MOTION_HINGES_V1 + 1,
    ));

    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"one-over"),
        FoldAssignment::Mountain,
        fixture.core_a[1],
        fixture.core_b[2],
        Point3::new(20_000.0, 0.0, 0.0).unwrap(),
        Point3::new(20_001.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let one_over = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        one_over_hinges,
    );
    assert!(recognize_bridge_edges_v1(&one_over, &fixture.audit).is_none());
}
