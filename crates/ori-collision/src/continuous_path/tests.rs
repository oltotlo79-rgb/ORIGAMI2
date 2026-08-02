use crate::prepare_tree_hinge_thickness_boundaries_v1;
use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    DyadicIntervalClosureLimitsV1, HalfAngleRationalEntryInputV1, RationalCoefficientV1,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;

fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
}

#[path = "tests/cooperative_control_tests.rs"]
mod cooperative_control_tests;
#[path = "tests/interval_tree_certificate_tests.rs"]
mod interval_tree_certificate_tests;
#[path = "tests/multi_cycle_authority_tests.rs"]
mod multi_cycle_authority_tests;
#[path = "tests/pair_proof_cache_cancellation_tests.rs"]
mod pair_proof_cache_cancellation_tests;
#[path = "tests/pair_proof_cache_invalidation_tests.rs"]
mod pair_proof_cache_invalidation_tests;
#[path = "tests/pair_proof_cache_production_tests.rs"]
mod pair_proof_cache_production_tests;

#[test]
fn wedge_prism_aabb_encloses_both_complete_rings_and_has_sound_exact_gap() {
    let iv = |lower, upper| ori_kinematics::OutwardIntervalV1::new(lower, upper).unwrap();
    let cell = SharedVertexWedgeCellV1 {
        pair: [fixed_id("8000", 1), fixed_id("8000", 2)],
        vertex: fixed_id("8000", 3),
        face: fixed_id("8000", 1),
        top_ring: vec![
            [iv(0.0, 0.1), iv(2.0, 2.1), iv(-1.0, -0.9)],
            [iv(1.0, 1.1), iv(2.0, 2.1), iv(0.0, 0.1)],
            [iv(0.0, 0.1), iv(2.0, 2.1), iv(1.0, 1.1)],
        ],
        bottom_ring: vec![
            [iv(0.0, 0.1), iv(-2.1, -2.0), iv(1.0, 1.1)],
            [iv(1.0, 1.1), iv(-2.1, -2.0), iv(0.0, 0.1)],
            [iv(0.0, 0.1), iv(-2.1, -2.0), iv(-1.0, -0.9)],
        ],
    };
    let mut work = 0;
    let bounds = wedge_cell_aabb_v1(&cell, &mut work, 18).unwrap();
    assert_eq!(work, 18);
    assert_eq!(
        wedge_cell_aabb_v1(&cell, &mut 0, 17),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    );
    assert_eq!(bounds, [[0.0, 1.1], [-2.1, 2.1], [-1.0, 1.1]]);
    let shifted = [[1.2, 2.0], [-2.1, 2.1], [-1.0, 1.1]];
    let lower = exact_common_axis_gap_lower_v1(&bounds, &shifted).unwrap();
    assert!(lower > 0.0);
    assert!(lower <= 0.1);
    let exact_gap = BigRational::from_f64(1.2).unwrap() - BigRational::from_f64(1.1).unwrap();
    assert!(BigRational::from_f64(lower).unwrap() <= exact_gap);
}

#[test]
fn exact_gap_helpers_keep_bit_exact_zero_and_integer_results() {
    let first = [[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]];
    let second = [[3.0, 4.0], [0.0, 1.0], [0.0, 1.0]];
    assert_eq!(
        exact_common_axis_gap_lower_v1(&first, &second)
            .expect("exact integer common-axis gap")
            .to_bits(),
        2.0_f64.to_bits()
    );

    let interval = |value| ori_kinematics::OutwardIntervalV1::new(value, value).unwrap();
    let origin = [interval(0.0); 3];
    let three_four_five = [interval(3.0), interval(4.0), interval(0.0)];
    assert_eq!(
        interval_point_distance_lower_v1(origin, three_four_five)
            .expect("exact 3-4-5 distance")
            .to_bits(),
        5.0_f64.to_bits()
    );
    assert_eq!(
        interval_point_distance_lower_v1(origin, origin)
            .expect("coincident interval distance")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn wedge_pair_keys_are_unique_and_canonical() {
    let iv = ori_kinematics::OutwardIntervalV1::new(0.0, 0.0).unwrap();
    let ring = vec![[iv; 3]; 3];
    let face_a: FaceId = fixed_id("8000", 1);
    let face_b: FaceId = fixed_id("8000", 2);
    let vertex = fixed_id("8000", 3);
    let cells = [face_b, face_a]
        .into_iter()
        .map(|face| SharedVertexWedgeCellV1 {
            pair: [face_a, face_b],
            vertex,
            face,
            top_ring: ring.clone(),
            bottom_ring: ring.clone(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        wedge_pair_keys_v1(&cells).unwrap(),
        vec![([face_a, face_b], vertex)]
    );
    let mut invalid = cells;
    invalid[0].pair = [face_b, face_a];
    assert_eq!(
        wedge_pair_keys_v1(&invalid),
        Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
    );
}

fn rational_cycle_bay_geometry(
    group_count: usize,
    reverse_hinges: bool,
) -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    ori_kinematics::CanonicalCycleScheduleV1,
    FaceId,
) {
    rational_cycle_bay_geometry_with_positive_constant(group_count, reverse_hinges, false)
}

fn rational_cycle_bay_geometry_with_positive_constant(
    group_count: usize,
    reverse_hinges: bool,
    positive_constant: bool,
) -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    ori_kinematics::CanonicalCycleScheduleV1,
    FaceId,
) {
    let triples = [
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
    ];
    let (pattern, paper, hinges) = if reverse_hinges && group_count == 16 {
        super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern_with_reversed_hinges(
        )
    } else if reverse_hinges && group_count == 8 {
        super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern_with_reversed_hinges()
    } else if reverse_hinges {
        super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern_with_reversed_hinges()
    } else if group_count == 32 {
        super::four_bay_cycle_test_support::thirty_two_bay_rational_cycle_pattern()
    } else if group_count == 16 {
        super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern()
    } else if group_count == 8 {
        super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern()
    } else {
        super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern()
    };
    let analysis = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b600", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    let topology = analysis
        .snapshot
        .unwrap_or_else(|| panic!("four non-crossing rational bays: {:?}", analysis.issues));
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = topology
        .faces
        .iter()
        .max_by_key(|face| {
            topology
                .hinge_adjacency
                .iter()
                .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                .count()
        })
        .unwrap()
        .id;
    let mut inputs = hinges
        .into_iter()
        .enumerate()
        .map(|(index, edge)| {
            let (p, q, _) = triples[(index / 4) % triples.len()];
            ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if positive_constant {
                    vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    }]
                } else {
                    vec![
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: if index % 2 == 0 { 1 } else { p as i64 },
                            denominator: 1,
                        },
                    ]
                },
                denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                    numerator: if index % 2 == 0 { 1 } else { q as i64 },
                    denominator: 1,
                }],
            }
        })
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        ori_kinematics::CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    (geometry, audit, schedule, fixed)
}

#[test]
fn four_non_crossing_rational_bays_admit_closure_but_not_sampled_clearance_authority() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    assert_eq!(geometry.hinges().len(), 16);
    assert_eq!(geometry.face_ids().len(), 13);
    assert!(geometry.face_ids().iter().all(|face| {
        geometry
            .face_boundary_vertices(*face)
            .is_some_and(|boundary| boundary.len() >= 3)
    }));
    for u in [0.0, 0.5, 1.0] {
        let angles = schedule.evaluate(u).unwrap();
        geometry
            .solve_closed(&audit, fixed, &angles, 1.0e-8)
            .unwrap();
    }
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
            },
        )
        .expect("four-leaf real-geometry closure");
    assert_eq!(closure.leaves().len(), 4);
    assert!(closure.every_leaf_covers_graph_v1(&geometry));
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule, &initial, &requested,
    )
    .unwrap();
    let groups =
        composed_symmetric_rational_local_groups_v1(&geometry, &audit, fixed, candidate.schedule())
            .unwrap();
    assert!(symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &groups
    ));
    let mut cross_leaf_collision = groups.clone();
    let foreign_face = *groups.iter().find(|(_, group)| **group == 3).unwrap().0;
    cross_leaf_collision.insert(foreign_face, 2);
    assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry,
        &cross_leaf_collision
    ));
    let diagnostic = diagnose_scheduled_cycle_path_v1(
        &geometry,
        &audit,
        fixed,
        &candidate,
        &closure,
        MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 - 1,
    );
    assert!(
        diagnostic.continuous_certificate_model_id().is_none(),
        "closure plus separated cactus-group bounds is not a continuous all-pair proof"
    );
    for cancelled_or_excessive in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
        assert!(
            diagnose_scheduled_cycle_path_v1(
                &geometry,
                &audit,
                fixed,
                &candidate,
                &closure,
                cancelled_or_excessive,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
    let retried =
        diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
    assert_eq!(retried, diagnostic);
    for thickness in [0.1, 1.0, 3.0] {
        let positive = diagnose_scheduled_positive_thickness_cycle_path_v1(
            &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
        );
        assert!(
            positive.continuous_certificate_model_id().is_none(),
            "finite endpoint/midpoint samples cannot mint positive-thickness authority"
        );
        for one_short_or_cancelled in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
            assert!(
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &candidate,
                    &closure,
                    thickness,
                    one_short_or_cancelled,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
    }
    for invalid_thickness in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        assert!(
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry,
                &audit,
                fixed,
                &candidate,
                &closure,
                invalid_thickness,
                32,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32,
        )
        .is_none(),
        "an uncertified all-pair path cannot issue a transition"
    );
    let (reversed_geometry, reversed_audit, reversed_schedule, reversed_fixed) =
        rational_cycle_bay_geometry(4, true);
    let reversed_closure = reversed_geometry
        .prove_dyadic_schedule_closure_v1(
            &reversed_audit,
            reversed_fixed,
            &reversed_schedule,
            1.0e-8,
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    let reversed_initial = reversed_schedule.evaluate(0.0).unwrap();
    let reversed_requested = reversed_schedule.evaluate(1.0).unwrap();
    let reversed_candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        reversed_schedule,
        &reversed_initial,
        &reversed_requested,
    )
    .unwrap();
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &reversed_geometry,
            &reversed_audit,
            reversed_fixed,
            &reversed_candidate,
            &reversed_closure,
            32,
        )
        .is_none(),
        "edge reversal cannot turn sampled evidence into authority"
    );
    for thickness in [0.1, 1.0, 3.0] {
        assert_eq!(
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
            ),
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &reversed_geometry,
                &reversed_audit,
                reversed_fixed,
                &reversed_candidate,
                &reversed_closure,
                thickness,
                32,
            )
        );
    }
}

#[test]
fn continuous_pair_gap_registry_exactly_enumerates_four_eight_sixteen_bays() {
    for bay_count in [4_usize, 8, 16] {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(bay_count, false);
        let registry = diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule)
            .expect("bounded pair registry");
        assert!(registry.is_for(&geometry, &audit, fixed, &schedule));
        let foreign_fixed = geometry
            .face_ids()
            .iter()
            .copied()
            .find(|face| *face != fixed)
            .unwrap();
        assert!(!registry.is_for(&geometry, &audit, foreign_fixed, &schedule));
        let (foreign_geometry, foreign_audit, foreign_schedule, foreign_fixed) =
            rational_cycle_bay_geometry(bay_count, true);
        assert!(!registry.is_for(
            &foreign_geometry,
            &foreign_audit,
            foreign_fixed,
            &foreign_schedule,
        ));
        assert!(!registry.authorizes_project_mutation());
        assert!(!registry.authorizes_continuous_motion());
        let face_count = geometry.face_ids().len();
        assert_eq!(registry.entries().len(), face_count * (face_count - 1) / 2);
        assert!(registry.entries().windows(2).all(|entries| {
            let left = entries[0].pair();
            let right = entries[1].pair();
            (left[0].canonical_bytes(), left[1].canonical_bytes())
                < (right[0].canonical_bytes(), right[1].canonical_bytes())
        }));
        assert!(registry.entries().iter().any(|entry| {
            entry.kind() == ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
        }));
        assert!(registry.entries().iter().any(|entry| {
            matches!(
                entry.kind(),
                ContinuousPairCoverageKindV1::SameGroupSkipped
                    | ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor
            )
        }));
        assert!(registry.gap_count() > 0);
        let corridor = diagnose_shared_hinge_continuous_corridor_gaps_v1(
            &registry, &geometry, &audit, fixed, &schedule, 0.1,
        )
        .expect("shared-hinge prerequisite gap report");
        assert!(corridor.is_for(&geometry, &audit, fixed, &schedule, 0.1));
        assert!(!corridor.authorizes_continuous_motion());
        assert!(!corridor.authorizes_project_mutation());
        assert_eq!(corridor.gaps().len(), geometry.hinges().len());
        assert!(corridor.gaps().iter().all(|gap| {
            registry.entries().iter().any(|entry| {
                entry.pair() == gap.pair()
                    && entry.kind() == ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
            })
        }));
        assert!(
            diagnose_shared_hinge_continuous_corridor_gaps_v1(
                &registry, &geometry, &audit, fixed, &schedule, 0.0,
            )
            .is_none()
        );
        let shared_vertices = diagnose_shared_vertex_continuous_corridor_gaps_v1(
            &registry, &geometry, &audit, fixed, &schedule, 0.1,
        )
        .expect("exact shared-vertex gap report");
        assert!(shared_vertices.is_for(&geometry, &audit, fixed, &schedule, 0.1));
        assert!(!shared_vertices.is_for(&geometry, &audit, fixed, &schedule, 0.2));
        assert!(!shared_vertices.authorizes_continuous_motion());
        assert!(!shared_vertices.authorizes_project_mutation());
        let expected_shared_vertices = registry
            .entries()
            .iter()
            .filter(|entry| entry.kind() == ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor)
            .count();
        assert_eq!(shared_vertices.gaps().len(), expected_shared_vertices);
        assert!(expected_shared_vertices > 0);
        assert!(shared_vertices.gaps().iter().all(|gap| {
            let first = geometry.face_boundary_vertices(gap.pair()[0]).unwrap();
            let second = geometry.face_boundary_vertices(gap.pair()[1]).unwrap();
            first
                .iter()
                .filter(|vertex| second.contains(vertex))
                .count()
                == 1
                && first.contains(&gap.vertex())
                && second.contains(&gap.vertex())
                && !geometry.hinges().iter().any(|hinge| {
                    [hinge.left_face(), hinge.right_face()] == gap.pair()
                        || [hinge.right_face(), hinge.left_face()] == gap.pair()
                })
        }));
        let mut vertex_policies = shared_vertices
            .gaps()
            .iter()
            .map(|gap| {
                let mut incident_faces = geometry
                    .face_ids()
                    .iter()
                    .copied()
                    .filter(|face| {
                        geometry
                            .face_boundary_vertices(*face)
                            .is_some_and(|vertices| vertices.contains(&gap.vertex()))
                    })
                    .collect::<Vec<_>>();
                incident_faces.sort_unstable_by_key(FaceId::canonical_bytes);
                crate::VertexReliefPolicyRecordV1 {
                    vertex: gap.vertex(),
                    cutout_radius_mm: 0.1,
                    material_thickness_mm: 0.1,
                    incident_faces,
                }
            })
            .collect::<Vec<_>>();
        vertex_policies.sort_unstable_by_key(|record| record.vertex.canonical_bytes());
        vertex_policies.dedup_by_key(|record| record.vertex);
        assert!(!vertex_policies.is_empty());
        assert!(shared_vertices.gaps().iter().all(|gap| {
            vertex_policies
                .iter()
                .any(|record| record.vertex == gap.vertex())
        }));
        let vertex_relief =
            crate::prepare_vertex_relief_prerequisite_v1(&geometry, 0.1, &vertex_policies)
                .expect("actual shared material vertex relief prerequisite");
        assert!(!vertex_relief.authorizes_shared_vertex_admission());
        assert!(!vertex_relief.authorizes_project_mutation());
        crate::revalidate_vertex_relief_prerequisite_v1(
            &vertex_relief,
            &geometry,
            0.1,
            &vertex_policies,
        )
        .unwrap();
        if !vertex_policies.is_empty() {
            vertex_policies[0].cutout_radius_mm = f64::from_bits(0.1_f64.to_bits() + 1);
            assert_eq!(
                crate::revalidate_vertex_relief_prerequisite_v1(
                    &vertex_relief,
                    &geometry,
                    0.1,
                    &vertex_policies,
                ),
                Err(crate::HingeReliefPolicyErrorV1::BindingMismatch)
            );
            vertex_policies[0].cutout_radius_mm = 0.1;
            vertex_policies[0].incident_faces.pop();
            assert_eq!(
                crate::revalidate_vertex_relief_prerequisite_v1(
                    &vertex_relief,
                    &geometry,
                    0.1,
                    &vertex_policies,
                ),
                Err(crate::HingeReliefPolicyErrorV1::VertexIncidentFacesMismatch)
            );
        }
    }
}

#[test]
fn sector_boundary_uses_xz_plane_y_normal_and_strict_local_radius() {
    let (geometry, _, _, _) = rational_cycle_bay_geometry(4, false);
    let face = geometry.face_ids()[0];
    let boundary = geometry.face_boundary_vertices(face).unwrap();
    let (vertex, other) = boundary
        .iter()
        .copied()
        .zip(boundary.iter().copied().cycle().skip(1))
        .take(boundary.len())
        .find(|(vertex, other)| {
            let a = geometry.vertex_position(*vertex).unwrap();
            let b = geometry.vertex_position(*other).unwrap();
            a.x() == b.x() || a.z() == b.z()
        })
        .unwrap();
    let origin = geometry.vertex_position(vertex).unwrap();
    let endpoint = geometry.vertex_position(other).unwrap();
    let edge = ((endpoint.x() - origin.x()).powi(2) + (endpoint.z() - origin.z()).powi(2)).sqrt();
    let radius = edge / 4.0;
    let points = sector_boundary_local_point(&geometry, vertex, other, radius, 0.2).unwrap();
    assert!(points[0][1].lower() <= -0.1 && points[0][1].upper() <= 0.0);
    assert!(points[1][1].lower() >= 0.0 && points[1][1].upper() >= 0.1);
    assert_eq!(
        (points[0][0].lower(), points[0][0].upper()),
        (points[1][0].lower(), points[1][0].upper())
    );
    assert_eq!(
        (points[0][2].lower(), points[0][2].upper()),
        (points[1][2].lower(), points[1][2].upper())
    );
    assert!(matches!(
        sector_boundary_local_point(&geometry, vertex, other, edge, 0.2),
        Err(DyadicFaceTransformIntervalErrorV1::Unproven)
    ));
    assert!(matches!(
        sector_boundary_local_point(
            &geometry,
            vertex,
            other,
            f64::from_bits(edge.to_bits() + 1),
            0.2
        ),
        Err(DyadicFaceTransformIntervalErrorV1::Unproven)
    ));
}

#[test]
fn exact_wedge_clip_covers_full_half_plane_and_enforces_one_short_work() {
    let r = |n: i64| BigRational::from_integer(n.into());
    let polygon = vec![[r(0), r(0)], [r(4), r(0)], [r(4), r(4)], [r(0), r(4)]];
    let mut work = 0;
    let mut meter = WedgeExactMeterV1::new(4).unwrap();
    let clipped = exact_clip_wedge_v1(
        &polygon,
        &[r(0), r(0)],
        &[r(1), r(1)],
        &r(2),
        &mut work,
        4,
        &mut meter,
    )
    .unwrap();
    assert_eq!(clipped.len(), 5);
    assert!(
        clipped
            .iter()
            .all(|p| &p[0] + &p[1] >= BigRational::from_integer(2.into()))
    );
    let mut one_short = 0;
    let mut meter = WedgeExactMeterV1::new(4).unwrap();
    assert!(matches!(
        exact_clip_wedge_v1(
            &polygon,
            &[r(0), r(0)],
            &[r(1), r(1)],
            &r(2),
            &mut one_short,
            3,
            &mut meter
        ),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    ));
    let mut measured_work = 0;
    let mut measured = WedgeExactMeterV1::with_bit_limit(usize::MAX);
    exact_clip_wedge_v1(
        &polygon,
        &[r(0), r(0)],
        &[r(1), r(1)],
        &r(2),
        &mut measured_work,
        4,
        &mut measured,
    )
    .unwrap();
    let exact_bit_work = measured.bit_work();
    let mut at_boundary_work = 0;
    let mut at_boundary = WedgeExactMeterV1::with_bit_limit(exact_bit_work);
    assert!(
        exact_clip_wedge_v1(
            &polygon,
            &[r(0), r(0)],
            &[r(1), r(1)],
            &r(2),
            &mut at_boundary_work,
            4,
            &mut at_boundary,
        )
        .is_ok()
    );
    let mut one_bit_short_work = 0;
    let mut one_bit_short = WedgeExactMeterV1::with_bit_limit(exact_bit_work - 1);
    assert!(matches!(
        exact_clip_wedge_v1(
            &polygon,
            &[r(0), r(0)],
            &[r(1), r(1)],
            &r(2),
            &mut one_bit_short_work,
            4,
            &mut one_bit_short,
        ),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    ));
    let huge = BigRational::new(num_bigint::BigInt::from(1_u8) << 8_193, 1.into());
    let mut bounded = 0;
    let mut meter = WedgeExactMeterV1::new(4).unwrap();
    assert!(matches!(
        exact_clip_wedge_v1(
            &polygon,
            &[r(0), r(0)],
            &[huge, r(1)],
            &r(2),
            &mut bounded,
            4,
            &mut meter
        ),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    ));
    let exact_limit = BigRational::from_integer(
        num_bigint::BigInt::from(1_u8) << (MAX_SHARED_VERTEX_WEDGE_BITS_V1 - 1),
    );
    assert_eq!(
        wedge_check_point_bits_v1(&[exact_limit.clone(), r(0)]),
        Ok(())
    );
    let one_bit_over = BigRational::from_integer(
        num_bigint::BigInt::from(1_u8) << MAX_SHARED_VERTEX_WEDGE_BITS_V1,
    );
    assert_eq!(
        wedge_check_point_bits_v1(&[one_bit_over, r(0)]),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    );
}

#[test]
fn shared_vertex_tree_layer_transport_peak_is_phase_exact_and_checked() {
    let source_retained_bytes = 101usize;
    let target_retained_bytes = 37usize;
    let shell = std::mem::size_of::<SharedVertexTreeLayerTransportProofV1>()
        - std::mem::size_of::<LayerOrderSnapshot>()
        - std::mem::size_of::<CanonicalHingeAngles>();
    let retained = checked_shared_vertex_tree_layer_transport_retained_bytes_v1(
        source_retained_bytes,
        target_retained_bytes,
    )
    .expect("bounded retained bytes");
    assert_eq!(
        retained,
        shell + source_retained_bytes + target_retained_bytes
    );
    assert_eq!(
        checked_shared_vertex_tree_layer_transport_peak_bytes_v1(
            source_retained_bytes,
            target_retained_bytes,
            target_retained_bytes,
        ),
        source_retained_bytes
            .checked_add(target_retained_bytes)
            .and_then(|bytes| bytes.checked_add(retained))
    );
    assert_eq!(
        checked_shared_vertex_tree_layer_transport_retained_bytes_v1(
            usize::MAX,
            target_retained_bytes,
        ),
        None
    );
    assert_eq!(
        checked_shared_vertex_tree_layer_transport_peak_bytes_v1(
            usize::MAX / 2 + 1,
            target_retained_bytes,
            target_retained_bytes,
        ),
        None
    );
    assert_eq!(
        checked_shared_vertex_tree_layer_transport_peak_bytes_v1(
            source_retained_bytes,
            usize::MAX,
            target_retained_bytes,
        ),
        None
    );
    assert_eq!(
        checked_canonical_hinge_angles_projected_retained_bytes_v1(usize::MAX),
        None
    );
}

#[test]
fn uniform_cycle_operation_count_accepts_exact_limit_and_rejects_overflow() {
    assert_eq!(
        checked_uniform_cycle_operation_count_v1(MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1,),
        MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1.checked_mul(64)
    );
    assert_eq!(
        checked_uniform_cycle_operation_count_v1(MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1 + 1,),
        None
    );
    assert_eq!(
        checked_uniform_cycle_operation_count_v1(usize::MAX / 64 + 1),
        None
    );
}

#[test]
fn positive_constant_actual_registries_compose_all_shared_hinge_relief_gaps() {
    for bay_count in [4_usize, 8, 16] {
        let (geometry, audit, schedule, fixed) =
            rational_cycle_bay_geometry_with_positive_constant(bay_count, false, true);
        let registry =
            diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule).unwrap();
        let gaps = diagnose_shared_hinge_continuous_corridor_gaps_v1(
            &registry, &geometry, &audit, fixed, &schedule, 0.1,
        )
        .unwrap();
        let mut policies = gaps
            .gaps()
            .iter()
            .map(|gap| HingeReliefPolicyRecordV1 {
                edge: gap.hinge(),
                cutout_width_mm: 7.0,
                bevel_angle_degrees: 1.0,
                material_thickness_mm: 0.1,
            })
            .collect::<Vec<_>>();
        policies.sort_unstable_by_key(|record| record.edge.canonical_bytes());
        let schedules = policies
            .iter()
            .map(|policy| {
                let gap = gaps
                    .gaps()
                    .iter()
                    .find(|gap| gap.hinge() == policy.edge)
                    .unwrap();
                HingeReliefLinearAngleScheduleV1 {
                    edge: policy.edge,
                    source_angle_degrees: f64::from_bits(gap.source_angle_bits()),
                    target_angle_degrees: f64::from_bits(gap.target_angle_bits()),
                }
            })
            .collect::<Vec<_>>();
        let limits = HingeReliefPolicyLimitsV1::default();
        let prerequisite =
            crate::prepare_hinge_relief_prerequisite_v1(&geometry, 0.1, &policies, limits).unwrap();
        let local = crate::certify_hinge_relief_local_intervals_v1(
            &prerequisite,
            &geometry,
            0.1,
            &policies,
            &schedules,
            limits,
        )
        .unwrap();
        let report = compose_shared_hinge_relief_coverage_v1(
            &registry,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            limits,
        )
        .unwrap();
        assert!(report.is_for_geometry(&geometry));
        assert!(report.is_for(&geometry, &audit, fixed, &schedule, 0.1));
        assert!(!report.is_for(&geometry, &audit, fixed, &schedule, 0.2));
        assert!(!report.authorizes_continuous_motion());
        assert!(!report.authorizes_project_mutation());
        assert_eq!(report.covered().len(), gaps.gaps().len());
        assert_eq!(
            report.covered().len() + report.remaining().len(),
            registry.entries().len()
        );
        let expected_remaining = registry
            .entries()
            .iter()
            .filter(|entry| entry.kind() != ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(report.remaining(), expected_remaining);
        assert!(report.remaining().iter().all(|entry| {
            entry.kind() != ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
        }));
        let mut expected_covered = gaps
            .gaps()
            .iter()
            .map(|gap| (gap.pair(), gap.hinge()))
            .collect::<Vec<_>>();
        expected_covered.sort_unstable_by_key(|(pair, hinge)| {
            (
                pair[0].canonical_bytes(),
                pair[1].canonical_bytes(),
                hinge.canonical_bytes(),
            )
        });
        let actual_covered = report
            .covered()
            .iter()
            .map(|item| (item.pair(), item.hinge()))
            .collect::<Vec<_>>();
        assert_eq!(actual_covered, expected_covered);

        if bay_count == 8 {
            let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(bay_count, false);
            let schedule_limits = ori_kinematics::CycleScheduleLimitsV1::default();
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    fixed,
                    &schedule,
                    1.0e-8,
                    ori_kinematics::DyadicIntervalClosureLimitsV1 {
                        max_depth: 3,
                        max_leaves: 8,
                        max_work: 1_000_000,
                        schedule_limits,
                    },
                )
                .unwrap();
            let transforms = prepare_dyadic_face_transform_interval_registry_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                1.0e-8,
                schedule_limits,
                16_777_216,
            )
            .unwrap_or_else(|error| panic!("bay {bay_count} transform registry: {error:?}"));
            assert!(!transforms.authorizes_continuous_motion());
            assert!(!transforms.authorizes_project_mutation());
            assert!(transforms.is_for(DyadicFaceTransformBindingInputV1 {
                geometry: &geometry,
                audit: &audit,
                fixed_face: fixed,
                schedule: &schedule,
                closure: &closure,
                thickness_mm: 0.1,
                tolerance: 1.0e-8,
                schedule_limits,
                max_work_per_leaf: 16_777_216,
            }));
            assert_eq!(transforms.leaves().len(), closure.leaves().len());
            assert!(transforms.leaves().iter().all(|leaf| {
                leaf.transforms().transforms().len() == geometry.face_ids().len()
                    && leaf
                        .transforms()
                        .transforms()
                        .windows(2)
                        .all(|pair| pair[0].0.canonical_bytes() < pair[1].0.canonical_bytes())
            }));
            assert!(!transforms.is_for(DyadicFaceTransformBindingInputV1 {
                geometry: &geometry,
                audit: &audit,
                fixed_face: fixed,
                schedule: &schedule,
                closure: &closure,
                thickness_mm: 0.2,
                tolerance: 1.0e-8,
                schedule_limits,
                max_work_per_leaf: 16_777_216,
            }));
            assert!(matches!(
                prepare_dyadic_face_transform_interval_registry_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &schedule,
                    &closure,
                    0.1,
                    1.0e-8,
                    schedule_limits,
                    1,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
            ));
            let pair_registry =
                diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule).unwrap();
            let vertex_gaps = diagnose_shared_vertex_continuous_corridor_gaps_v1(
                &pair_registry,
                &geometry,
                &audit,
                fixed,
                &schedule,
                0.1,
            )
            .unwrap();
            let binding = || DyadicFaceTransformBindingInputV1 {
                geometry: &geometry,
                audit: &audit,
                fixed_face: fixed,
                schedule: &schedule,
                closure: &closure,
                thickness_mm: 0.1,
                tolerance: 1.0e-8,
                schedule_limits,
                max_work_per_leaf: 16_777_216,
            };
            let vertex_positions = diagnose_dyadic_shared_vertex_interval_positions_v1(
                &transforms,
                &vertex_gaps,
                binding(),
                16_777_216,
            )
            .unwrap();
            let mut vertex_records = vertex_gaps
                .gaps()
                .iter()
                .map(|gap| {
                    let mut incident_faces = geometry
                        .face_ids()
                        .iter()
                        .copied()
                        .filter(|face| {
                            geometry
                                .face_boundary_vertices(*face)
                                .is_some_and(|vertices| vertices.contains(&gap.vertex()))
                        })
                        .collect::<Vec<_>>();
                    incident_faces.sort_unstable_by_key(FaceId::canonical_bytes);
                    crate::VertexReliefPolicyRecordV1 {
                        vertex: gap.vertex(),
                        cutout_radius_mm: 0.1,
                        material_thickness_mm: 0.1,
                        incident_faces,
                    }
                })
                .collect::<Vec<_>>();
            vertex_records.sort_unstable_by_key(|record| record.vertex.canonical_bytes());
            vertex_records.dedup_by_key(|record| record.vertex);
            let vertex_relief =
                crate::prepare_vertex_relief_prerequisite_v1(&geometry, 0.1, &vertex_records)
                    .unwrap();
            let sector_boundaries = diagnose_dyadic_shared_vertex_sector_boundaries_v1(
                &transforms,
                &vertex_gaps,
                &vertex_relief,
                &vertex_records,
                binding(),
                16_777_216,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "sector diagnostic gaps={} incident={} local={} leaves={}: {error:?}",
                    vertex_gaps.gaps().len(),
                    vertex_records
                        .iter()
                        .map(|record| record.incident_faces.len())
                        .sum::<usize>(),
                    vertex_gaps
                        .gaps()
                        .iter()
                        .map(|gap| vertex_records
                            .iter()
                            .find(|record| record.vertex == gap.vertex())
                            .unwrap()
                            .incident_faces
                            .len())
                        .sum::<usize>(),
                    transforms.leaves().len(),
                )
            });
            assert!(!sector_boundaries.authorizes_continuous_motion());
            assert!(!sector_boundaries.authorizes_project_mutation());
            assert!(sector_boundaries.is_for(
                &transforms,
                &vertex_gaps,
                &vertex_relief,
                &vertex_records,
                binding(),
                16_777_216,
            ));
            assert!(!sector_boundaries.is_for(
                &transforms,
                &vertex_gaps,
                &vertex_relief,
                &vertex_records,
                binding(),
                16_777_215,
            ));
            let mut tampered_sector = sector_boundaries.clone();
            tampered_sector.leaves[0].0 ^= 1;
            assert!(!tampered_sector.is_for(
                &transforms,
                &vertex_gaps,
                &vertex_relief,
                &vertex_records,
                binding(),
                16_777_216,
            ));
            let mut tampered_sector = sector_boundaries.clone();
            tampered_sector.leaves[0].2[1].pair = tampered_sector.leaves[0].2[0].pair;
            tampered_sector.leaves[0].2[1].vertex = tampered_sector.leaves[0].2[0].vertex;
            tampered_sector.leaves[0].2[1].face = tampered_sector.leaves[0].2[0].face;
            assert!(!tampered_sector.is_for(
                &transforms,
                &vertex_gaps,
                &vertex_relief,
                &vertex_records,
                binding(),
                16_777_216,
            ));
            assert!(matches!(
                diagnose_dyadic_shared_vertex_sector_boundaries_v1(
                    &transforms,
                    &vertex_gaps,
                    &vertex_relief,
                    &vertex_records,
                    binding(),
                    0,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
            ));
            assert!(matches!(
                diagnose_dyadic_shared_vertex_wedges_v1(
                    &sector_boundaries,
                    &transforms,
                    &vertex_gaps,
                    &vertex_relief,
                    &vertex_records,
                    binding(),
                    1_000_000,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::Unproven)
            ));
            let point_distances = diagnose_dyadic_shared_vertex_boundary_point_distances_v1(
                &sector_boundaries,
                &vertex_gaps,
                binding(),
                2_048,
            )
            .unwrap();
            assert!(!point_distances.authorizes_continuous_motion());
            assert!(!point_distances.authorizes_project_mutation());
            assert!(point_distances.is_for(&sector_boundaries, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.schedule_hash[0] ^= 1;
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.closure_hash[0] ^= 1;
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.thickness_bits ^= 1;
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.radius_binding[0].1 ^= 1;
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.max_work_per_point -= 1;
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            let mut foreign_sector = sector_boundaries.clone();
            foreign_sector.leaves[0].2[0].boundary[0][0][0] =
                ori_kinematics::OutwardIntervalV1::new(0.0, 0.0).unwrap();
            assert!(!point_distances.is_for(&foreign_sector, &vertex_gaps, binding(), 2_048,));
            assert!(point_distances.leaves().iter().all(|(_, _, bounds)| {
                bounds
                    .iter()
                    .all(|bound| bound.lower_mm().is_finite() && bound.lower_mm() >= 0.0)
            }));
            assert!(matches!(
                diagnose_dyadic_shared_vertex_boundary_point_distances_v1(
                    &sector_boundaries,
                    &vertex_gaps,
                    binding(),
                    0,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
            ));
            if transforms.leaves().len() * vertex_gaps.gaps().len() * 16 > 1 {
                assert!(matches!(
                    diagnose_dyadic_shared_vertex_boundary_point_distances_v1(
                        &sector_boundaries,
                        &vertex_gaps,
                        binding(),
                        1,
                    ),
                    Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
                ));
            }
            assert_eq!(sector_boundaries.leaves().len(), transforms.leaves().len());
            assert!(sector_boundaries.leaves().iter().all(|(_, _, entries)| {
                entries.iter().all(|entry| {
                    entry.boundary().iter().flatten().flatten().all(|value| {
                        value.lower().is_finite()
                            && value.upper().is_finite()
                            && value.lower() <= value.upper()
                    })
                })
            }));
            assert!(!vertex_positions.authorizes_continuous_motion());
            assert!(!vertex_positions.authorizes_project_mutation());
            assert!(vertex_positions.is_for(&transforms, &vertex_gaps, binding(), 16_777_216,));
            assert!(!vertex_positions.is_for(
                &transforms,
                &vertex_gaps,
                DyadicFaceTransformBindingInputV1 {
                    thickness_mm: 0.2,
                    ..binding()
                },
                16_777_216,
            ));
            assert!(!vertex_positions.is_for(
                &transforms,
                &vertex_gaps,
                DyadicFaceTransformBindingInputV1 {
                    tolerance: f64::from_bits(1.0e-8_f64.to_bits() + 1),
                    ..binding()
                },
                16_777_216,
            ));
            assert!(!vertex_positions.is_for(
                &transforms,
                &vertex_gaps,
                DyadicFaceTransformBindingInputV1 {
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                        max_hinges: schedule_limits.max_hinges - 1,
                        ..schedule_limits
                    },
                    ..binding()
                },
                16_777_216,
            ));
            assert!(!vertex_positions.is_for(
                &transforms,
                &vertex_gaps,
                DyadicFaceTransformBindingInputV1 {
                    max_work_per_leaf: 16_777_215,
                    ..binding()
                },
                16_777_216,
            ));
            assert!(!vertex_positions.is_for(&transforms, &vertex_gaps, binding(), 16_777_215,));
            assert_eq!(vertex_positions.leaves().len(), transforms.leaves().len());
            assert!(vertex_positions.leaves().iter().all(|leaf| {
                leaf.positions().len() == vertex_gaps.gaps().len()
                    && leaf
                        .positions()
                        .iter()
                        .zip(vertex_gaps.gaps())
                        .all(|(position, gap)| {
                            position.pair() == gap.pair() && position.vertex() == gap.vertex()
                        })
            }));
            assert!(matches!(
                diagnose_dyadic_shared_vertex_interval_positions_v1(
                    &transforms,
                    &vertex_gaps,
                    binding(),
                    0,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
            ));
            assert!(matches!(
                diagnose_dyadic_shared_vertex_interval_positions_v1(
                    &transforms,
                    &vertex_gaps,
                    binding(),
                    1,
                ),
                Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
            ));
        }

        let mut tampered = schedules.clone();
        tampered[0].source_angle_degrees =
            f64::from_bits(tampered[0].source_angle_degrees.to_bits() + 1);
        assert!(matches!(
            compose_shared_hinge_relief_coverage_v1(
                &registry,
                &geometry,
                &audit,
                fixed,
                &schedule,
                0.1,
                &prerequisite,
                &local,
                &policies,
                &tampered,
                limits,
            ),
            Err(SharedHingeReliefCoverageErrorV1::ForeignRelief)
        ));
    }
}

#[test]
fn continuous_pair_gap_classifier_fails_closed_without_metadata_and_at_cap() {
    assert_eq!(
        classify_continuous_pair_v1(1, None, None),
        ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
    );
    assert_eq!(
        classify_continuous_pair_v1(MAX_SHARED_HINGES_PER_CONTINUOUS_PAIR_V1, None, None),
        ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
    );
    assert_eq!(
        classify_continuous_pair_v1(0, Some(false), None),
        ContinuousPairCoverageKindV1::MetadataMissing
    );
    assert_eq!(
        classify_continuous_pair_v1(0, Some(false), Some((Some(3), Some(3)))),
        ContinuousPairCoverageKindV1::SameGroupSkipped
    );
    assert_eq!(
        classify_continuous_pair_v1(0, Some(false), Some((Some(3), Some(4)))),
        ContinuousPairCoverageKindV1::ExistingNonhingeIntervalCandidate
    );
    assert_eq!(checked_unordered_pair_count_v1(65), Some(2_080));
    assert_eq!(checked_unordered_pair_count_v1(66), Some(2_145));
    assert!(checked_unordered_pair_count_v1(usize::MAX).is_none());
}

#[test]
fn relief_gap_schedule_matching_is_complete_at_four_eight_sixteen() {
    for count in [4_usize, 8, 16] {
        let gaps = (0..count)
            .map(|index| SharedHingeContinuousCorridorGapV1 {
                pair: [
                    fixed_id("b601", index as u64 * 2 + 1),
                    fixed_id("b601", index as u64 * 2 + 2),
                ],
                hinge: fixed_id("9601", index as u64 + 1),
                source_angle_bits: 90.0_f64.to_bits(),
                target_angle_bits: 120.0_f64.to_bits(),
                derivative_bound_bits: 30.0_f64.to_bits(),
                triangular_prerequisite: true,
            })
            .collect::<Vec<_>>();
        let schedules = gaps
            .iter()
            .map(|gap| HingeReliefLinearAngleScheduleV1 {
                edge: gap.hinge,
                source_angle_degrees: 90.0,
                target_angle_degrees: 120.0,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            match_relief_gap_schedules(&gaps, &schedules, |_| false)
                .unwrap()
                .len(),
            count
        );

        let mut tampered = schedules.clone();
        tampered[0].source_angle_degrees = f64::from_bits(90.0_f64.to_bits() + 1);
        assert_eq!(
            match_relief_gap_schedules(&gaps, &tampered, |_| false),
            Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage)
        );
        assert_eq!(
            match_relief_gap_schedules(&gaps, &schedules[..count - 1], |_| false),
            Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage)
        );
        let mut duplicate = schedules.clone();
        duplicate[1].edge = duplicate[0].edge;
        assert_eq!(
            match_relief_gap_schedules(&gaps, &duplicate, |_| false),
            Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage)
        );
    }
}

#[test]
fn relief_gap_schedule_matching_accepts_exact_total_cap_and_resource_limits_one_over() {
    let gap = |index: usize| SharedHingeContinuousCorridorGapV1 {
        pair: [
            fixed_id("b602", index as u64 * 2 + 1),
            fixed_id("b602", index as u64 * 2 + 2),
        ],
        hinge: fixed_id("9602", index as u64 + 1),
        source_angle_bits: 90.0_f64.to_bits(),
        target_angle_bits: 120.0_f64.to_bits(),
        derivative_bound_bits: 30.0_f64.to_bits(),
        triangular_prerequisite: true,
    };
    let gaps = (0..crate::MAX_HINGE_RELIEF_RECORDS_V1)
        .map(gap)
        .collect::<Vec<_>>();
    let schedules = gaps
        .iter()
        .map(|gap| HingeReliefLinearAngleScheduleV1 {
            edge: gap.hinge,
            source_angle_degrees: 90.0,
            target_angle_degrees: 120.0,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        match_relief_gap_schedules(&gaps, &schedules, |_| false)
            .expect("the exact total hinge limit remains covered")
            .len(),
        crate::MAX_HINGE_RELIEF_RECORDS_V1
    );

    let mut over_gaps = gaps;
    over_gaps.push(gap(crate::MAX_HINGE_RELIEF_RECORDS_V1));
    let mut over_schedules = schedules;
    over_schedules.push(HingeReliefLinearAngleScheduleV1 {
        edge: over_gaps.last().expect("one-over gap").hinge,
        source_angle_degrees: 90.0,
        target_angle_degrees: 120.0,
    });
    assert_eq!(
        match_relief_gap_schedules(&over_gaps, &over_schedules, |_| false),
        Err(SharedHingeReliefCoverageErrorV1::ResourceLimit)
    );
}

#[test]
fn cactus_star_groups_three_or_more_cycles_around_an_articulation_face() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    let exclusive = geometry
        .face_ids()
        .iter()
        .copied()
        .min_by_key(|face| {
            geometry
                .hinges()
                .iter()
                .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                .count()
        })
        .unwrap();
    let groups = rational_cactus_star_local_groups_v1(&geometry, &audit, exclusive, &schedule)
        .expect("four-cycle cactus block-cut star");
    assert_eq!(groups.len(), 12);
    assert_eq!(groups.values().copied().collect::<HashSet<_>>().len(), 4);
    assert!(symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &groups
    ));
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 3,
                max_leaves: 8,
                max_work: 8,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, 0.1, 32,
        )
        .is_none(),
        "cactus-group recognition and separated group bounds cannot mint all-pair continuous authority"
    );
}

#[test]
fn two_patch_miura_cactus_has_native_layer_authority() {
    let (pattern, paper, _) = crate::miura_cactus_test_support::two_patch_miura_cactus_pattern();
    let project = fixed_id("ca20", 1);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("seven-cell cactus topology");
    assert_eq!(topology.faces.len(), 7);
    assert_eq!(topology.hinge_adjacency.len() + 1 - topology.faces.len(), 2);
    let articulation = topology
        .faces
        .iter()
        .find(|face| {
            topology
                .hinge_adjacency
                .iter()
                .filter(|hinge| hinge.first == face.id || hinge.second == face.id)
                .count()
                == 4
        })
        .expect("central articulation face")
        .id;
    let mut remaining = topology
        .faces
        .iter()
        .map(|face| face.id)
        .filter(|face| *face != articulation)
        .collect::<HashSet<_>>();
    let mut component_count = 0;
    while let Some(seed) = remaining.iter().next().copied() {
        component_count += 1;
        remaining.remove(&seed);
        let mut frontier = vec![seed];
        while let Some(face) = frontier.pop() {
            for next in topology.hinge_adjacency.iter().filter_map(|hinge| {
                (hinge.first == face)
                    .then_some(hinge.second)
                    .or_else(|| (hinge.second == face).then_some(hinge.first))
            }) {
                if next != articulation && remaining.remove(&next) {
                    frontier.push(next);
                }
            }
        }
    }
    assert_eq!(component_count, 2);
    let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
    let global = ori_foldability::analyze_global_flat_foldability(
        ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
            project, &paper, &pattern, &topology, &local,
        ),
        ori_foldability::GlobalFlatFoldabilityLimits::default(),
    )
    .unwrap();
    assert!(global.layer_order().is_some(), "{:?}", global.outcome_v2());
}

#[test]
fn three_by_three_blocks_issue_canonical_blockwise_closure() {
    let project = fixed_id("ca40", 1);
    let blocks = crate::miura_cactus_test_support::independent_three_by_three_miura_blocks();
    let prepared = blocks.map(|(pattern, paper, moving)| {
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: project,
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .unwrap();
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
        let global = ori_foldability::analyze_global_flat_foldability(
            ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
                project, &paper, &pattern, &topology, &local,
            ),
            ori_foldability::GlobalFlatFoldabilityLimits::default(),
        )
        .unwrap();
        let source = global.layer_order().expect("native layer order").clone();
        (pattern, geometry, audit, moving, source)
    });
    let [
        (first_pattern, first_geometry, first_audit, first_moving, first_source),
        (second_pattern, second_geometry, second_audit, second_moving, second_source),
    ] = prepared;
    let shared = first_geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|face| second_geometry.face_ids().contains(face))
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 1);
    let articulation = shared[0];
    let make = |pattern: &CreasePattern,
                geometry: &MaterialHingeGraphGeometry,
                audit: &MaterialHingeGraphAudit,
                moving: Vec<EdgeId>| {
        let mut rows = moving
            .iter()
            .filter_map(|edge| {
                let source = pattern.edges.iter().find(|source| source.id == *edge)?;
                let start = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == source.start)?;
                Some(start.position.y.to_bits())
            })
            .collect::<Vec<_>>();
        rows.sort_unstable();
        rows.dedup();
        rows.into_iter()
            .find_map(|row| {
                let active = moving
                    .iter()
                    .copied()
                    .filter(|edge| {
                        let source = pattern
                            .edges
                            .iter()
                            .find(|source| source.id == *edge)
                            .unwrap();
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == source.start)
                            .unwrap()
                            .position
                            .y
                            .to_bits()
                            == row
                    })
                    .collect::<HashSet<_>>();
                let mut entries = geometry
                    .hinges()
                    .iter()
                    .map(|hinge| HalfAngleRationalEntryInputV1 {
                        edge: hinge.edge(),
                        u_domain: [
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ],
                        numerator_power_coefficients: vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                        ],
                        denominator_power_coefficients: vec![RationalCoefficientV1 {
                            numerator: if active.contains(&hinge.edge()) {
                                64
                            } else {
                                1
                            },
                            denominator: 1,
                        }],
                    })
                    .collect::<Vec<_>>();
                entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
                let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    geometry,
                    audit,
                    articulation,
                    entries,
                    CycleScheduleLimitsV1::default(),
                )
                .ok()?;
                let closure = geometry
                    .prove_dyadic_schedule_closure_v1(
                        audit,
                        articulation,
                        &schedule,
                        1.0e-9,
                        DyadicIntervalClosureLimitsV1 {
                            max_depth: 8,
                            max_leaves: 256,
                            max_work: 1_000_000,
                            schedule_limits: CycleScheduleLimitsV1::default(),
                        },
                    )
                    .ok()?;
                Some((schedule, closure))
            })
            .expect("one canonical carrier closes")
    };
    let (first_schedule, first_closure) =
        make(&first_pattern, &first_geometry, &first_audit, first_moving);
    let (second_schedule, second_closure) = make(
        &second_pattern,
        &second_geometry,
        &second_audit,
        second_moving,
    );
    let authority = crate::issue_blockwise_closure_authority_v1(
        [
            crate::BlockwiseClosureInputV1 {
                geometry: &first_geometry,
                audit: &first_audit,
                schedule: &first_schedule,
                closure: &first_closure,
            },
            crate::BlockwiseClosureInputV1 {
                geometry: &second_geometry,
                audit: &second_audit,
                schedule: &second_schedule,
                closure: &second_closure,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
    )
    .unwrap();
    assert!(authority.revalidates_v1(articulation, 0.1, [0x61; 32]));
    assert!(!authority.revalidates_v1(articulation, 0.1, [0x60; 32]));
    assert!(!authority.revalidates_v1(FaceId::new(), 0.1, [0x61; 32]));
    assert!(!authority.revalidates_v1(articulation, 1.0, [0x61; 32]));
    let reordered = crate::issue_blockwise_closure_authority_v1(
        [
            crate::BlockwiseClosureInputV1 {
                geometry: &second_geometry,
                audit: &second_audit,
                schedule: &second_schedule,
                closure: &second_closure,
            },
            crate::BlockwiseClosureInputV1 {
                geometry: &first_geometry,
                audit: &first_audit,
                schedule: &first_schedule,
                closure: &first_closure,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
    )
    .unwrap();
    assert_eq!(
        authority.binding_fingerprint_v1(),
        reordered.binding_fingerprint_v1()
    );

    let first_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &first_geometry,
        &first_audit,
        articulation,
        &first_schedule,
        &first_closure,
        0.1,
        32,
    )
    .unwrap();
    let second_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &second_geometry,
        &second_audit,
        articulation,
        &second_schedule,
        &second_closure,
        0.1,
        32,
    )
    .unwrap();
    let make_layer = |geometry: &MaterialHingeGraphGeometry,
                      audit: &MaterialHingeGraphAudit,
                      source: &LayerOrderSnapshot,
                      schedule: &CanonicalCycleScheduleV1,
                      closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
                      positive: &PositiveThicknessContinuousCertificateV1| {
        crate::certify_general_multi_face_cell_transport_v1(crate::GeneralCellTransportInputV1 {
            geometry,
            audit,
            source,
            schedule,
            closure,
            positive_continuous: positive,
            paper_thickness_mm: 0.1,
            tolerance: crate::GENERAL_CELL_TRANSPORT_TOLERANCE_V1,
            limits: crate::GeneralCellTransportLimitsV1 {
                max_transitions: closure.leaves().len() + 1,
                max_cells: 1_000_000,
                max_layer_records: 1_000_000,
                max_boundary_samples: 1_000_000,
            },
        })
        .unwrap()
    };
    let first_layer = make_layer(
        &first_geometry,
        &first_audit,
        &first_source,
        &first_schedule,
        &first_closure,
        &first_positive,
    );
    let second_layer = make_layer(
        &second_geometry,
        &second_audit,
        &second_source,
        &second_schedule,
        &second_closure,
        &second_positive,
    );
    let parent = crate::issue_blockwise_closure_authority_v1(
        [
            crate::BlockwiseClosureInputV1 {
                geometry: &first_geometry,
                audit: &first_audit,
                schedule: &first_schedule,
                closure: &first_closure,
            },
            crate::BlockwiseClosureInputV1 {
                geometry: &second_geometry,
                audit: &second_audit,
                schedule: &second_schedule,
                closure: &second_closure,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
    )
    .unwrap();
    let composed = crate::issue_blockwise_positive_layer_authority_v1(
        parent,
        [
            crate::BlockwisePositiveLayerInputV1 {
                source: &first_source,
                positive: first_positive,
                layer: first_layer,
            },
            crate::BlockwisePositiveLayerInputV1 {
                source: &second_source,
                positive: second_positive,
                layer: second_layer,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
        [0x71; 32],
    )
    .unwrap();
    let mut target_angles = first_schedule
        .evaluate(1.0)
        .unwrap()
        .as_slice()
        .iter()
        .chain(second_schedule.evaluate(1.0).unwrap().as_slice().iter())
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    assert!(composed.target_angles_match_v1(&target_angles));
    target_angles[0].1 = f64::from_bits(target_angles[0].1.to_bits() ^ 1);
    assert!(!composed.target_angles_match_v1(&target_angles));
    assert!(composed.revalidates_v1(
        [&first_source, &second_source],
        articulation,
        0.1,
        [0x61; 32],
        [0x71; 32]
    ));
    assert!(!composed.revalidates_v1(
        [&first_source, &second_source],
        articulation,
        0.1,
        [0x61; 32],
        [0x70; 32]
    ));
    assert!(!composed.revalidates_v1(
        [&first_source, &second_source],
        articulation,
        0.1,
        [0x60; 32],
        [0x71; 32]
    ));
    assert!(!composed.revalidates_v1(
        [&first_source, &second_source],
        FaceId::new(),
        0.1,
        [0x61; 32],
        [0x71; 32]
    ));
    assert!(!composed.revalidates_v1(
        [&first_source, &second_source],
        articulation,
        1.0,
        [0x61; 32],
        [0x71; 32]
    ));
    assert!(!composed.revalidates_v1(
        [&second_source, &first_source],
        articulation,
        0.1,
        [0x61; 32],
        [0x71; 32]
    ));
    let substituted_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &first_geometry,
        &first_audit,
        articulation,
        &first_schedule,
        &first_closure,
        0.1,
        32,
    )
    .unwrap();
    let substituted_layer = make_layer(
        &first_geometry,
        &first_audit,
        &first_source,
        &first_schedule,
        &first_closure,
        &substituted_positive,
    );
    let duplicated_substituted_layer = make_layer(
        &first_geometry,
        &first_audit,
        &first_source,
        &first_schedule,
        &first_closure,
        &substituted_positive,
    );
    let substitution_parent = crate::issue_blockwise_closure_authority_v1(
        [
            crate::BlockwiseClosureInputV1 {
                geometry: &first_geometry,
                audit: &first_audit,
                schedule: &first_schedule,
                closure: &first_closure,
            },
            crate::BlockwiseClosureInputV1 {
                geometry: &second_geometry,
                audit: &second_audit,
                schedule: &second_schedule,
                closure: &second_closure,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
    )
    .unwrap();
    assert!(
        crate::issue_blockwise_positive_layer_authority_v1(
            substitution_parent,
            [
                crate::BlockwisePositiveLayerInputV1 {
                    source: &first_source,
                    positive: substituted_positive.clone(),
                    layer: duplicated_substituted_layer,
                },
                crate::BlockwisePositiveLayerInputV1 {
                    source: &second_source,
                    positive: substituted_positive,
                    layer: substituted_layer,
                },
            ],
            articulation,
            0.1,
            [0x61; 32],
            [0x71; 32],
        )
        .is_none()
    );
    let reordered_second_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &second_geometry,
        &second_audit,
        articulation,
        &second_schedule,
        &second_closure,
        0.1,
        32,
    )
    .unwrap();
    let reordered_second_layer = make_layer(
        &second_geometry,
        &second_audit,
        &second_source,
        &second_schedule,
        &second_closure,
        &reordered_second_positive,
    );
    let reordered_first_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &first_geometry,
        &first_audit,
        articulation,
        &first_schedule,
        &first_closure,
        0.1,
        32,
    )
    .unwrap();
    let reordered_first_layer = make_layer(
        &first_geometry,
        &first_audit,
        &first_source,
        &first_schedule,
        &first_closure,
        &reordered_first_positive,
    );
    let reordered_parent = crate::issue_blockwise_closure_authority_v1(
        [
            crate::BlockwiseClosureInputV1 {
                geometry: &second_geometry,
                audit: &second_audit,
                schedule: &second_schedule,
                closure: &second_closure,
            },
            crate::BlockwiseClosureInputV1 {
                geometry: &first_geometry,
                audit: &first_audit,
                schedule: &first_schedule,
                closure: &first_closure,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
    )
    .unwrap();
    let reordered_composed = crate::issue_blockwise_positive_layer_authority_v1(
        reordered_parent,
        [
            crate::BlockwisePositiveLayerInputV1 {
                source: &second_source,
                positive: reordered_second_positive,
                layer: reordered_second_layer,
            },
            crate::BlockwisePositiveLayerInputV1 {
                source: &first_source,
                positive: reordered_first_positive,
                layer: reordered_first_layer,
            },
        ],
        articulation,
        0.1,
        [0x61; 32],
        [0x71; 32],
    )
    .unwrap();
    assert_eq!(
        composed.binding_fingerprint_v1(),
        reordered_composed.binding_fingerprint_v1()
    );
    assert!(
        crate::issue_blockwise_closure_authority_v1(
            [
                crate::BlockwiseClosureInputV1 {
                    geometry: &first_geometry,
                    audit: &first_audit,
                    schedule: &first_schedule,
                    closure: &first_closure,
                },
                crate::BlockwiseClosureInputV1 {
                    geometry: &first_geometry,
                    audit: &first_audit,
                    schedule: &first_schedule,
                    closure: &first_closure,
                },
            ],
            articulation,
            0.1,
            [0x61; 32],
        )
        .is_none()
    );
    assert!(
        crate::issue_blockwise_closure_authority_v1(
            [
                crate::BlockwiseClosureInputV1 {
                    geometry: &first_geometry,
                    audit: &first_audit,
                    schedule: &first_schedule,
                    closure: &first_closure,
                },
                crate::BlockwiseClosureInputV1 {
                    geometry: &second_geometry,
                    audit: &second_audit,
                    schedule: &first_schedule,
                    closure: &first_closure,
                },
            ],
            articulation,
            0.1,
            [0x61; 32],
        )
        .is_none()
    );
}

#[test]
fn eight_bay_real_geometry_admits_closure_but_not_sampled_clearance_authority() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(8, false);
    let limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
        max_depth: 3,
        max_leaves: 8,
        max_work: 8,
        schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
    };
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, limits)
        .unwrap();
    assert_eq!(geometry.hinges().len(), 32);
    assert_eq!(closure.leaves().len(), 8);
    assert!(closure.leaves().iter().all(|leaf| leaf.0 == 3));
    for short in [
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_depth: 2,
            ..limits
        },
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_leaves: 7,
            ..limits
        },
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_work: 7,
            ..limits
        },
    ] {
        assert_eq!(
            geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, short,),
            Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
        );
    }
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule, &initial, &requested,
    )
    .unwrap();
    let groups =
        composed_symmetric_rational_local_groups_v1(&geometry, &audit, fixed, candidate.schedule())
            .unwrap();
    assert!(symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &groups
    ));
    let mut collision = groups.clone();
    let foreign = *groups.iter().find(|(_, group)| **group == 7).unwrap().0;
    collision.insert(foreign, 6);
    assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &collision
    ));
    let diagnostic =
        diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
    assert!(diagnostic.continuous_certificate_model_id().is_none());
    for denied in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
        assert!(
            diagnose_scheduled_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, denied,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32,
        )
        .is_none()
    );
    let (rg, ra, rs, rf) = rational_cycle_bay_geometry(8, true);
    let rc = rg
        .prove_dyadic_schedule_closure_v1(&ra, rf, &rs, 1.0e-8, limits)
        .unwrap();
    let ri = rs.evaluate(0.0).unwrap();
    let rr = rs.evaluate(1.0).unwrap();
    let reversed_candidate =
        ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(rs, &ri, &rr).unwrap();
    assert!(
        crate::certify_scheduled_cycle_transition_v1(&rg, &ra, rf, &reversed_candidate, &rc, 32,)
            .is_none()
    );
    for thickness in [0.1, 1.0, 3.0] {
        assert_eq!(
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
            ),
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &rg,
                &ra,
                rf,
                &reversed_candidate,
                &rc,
                thickness,
                32,
            )
        );
    }
}

#[test]
fn sixteen_bay_geometry_closes_at_exact_caps_without_sampled_clearance_authority() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(16, false);
    let limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
        max_depth: 4,
        max_leaves: 16,
        max_work: 16,
        schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
    };
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, limits)
        .unwrap();
    assert_eq!(geometry.hinges().len(), 64);
    assert_eq!(closure.leaves().len(), 16);
    assert!(closure.leaves().iter().all(|leaf| leaf.0 == 4));
    for short in [
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_depth: 3,
            ..limits
        },
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_leaves: 15,
            ..limits
        },
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_work: 15,
            ..limits
        },
    ] {
        assert_eq!(
            geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, short,),
            Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
        );
    }
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule, &initial, &requested,
    )
    .unwrap();
    let groups =
        composed_symmetric_rational_local_groups_v1(&geometry, &audit, fixed, candidate.schedule())
            .unwrap();
    assert!(symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &groups
    ));
    let mut collision = groups.clone();
    let foreign = *groups.iter().find(|(_, group)| **group == 15).unwrap().0;
    collision.insert(foreign, 14);
    assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
        &geometry, &collision
    ));
    let diagnostic =
        diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
    assert!(diagnostic.continuous_certificate_model_id().is_none());
    for denied in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
        assert!(
            diagnose_scheduled_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, denied,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32,
        )
        .is_none()
    );
    let (rg, ra, rs, rf) = rational_cycle_bay_geometry(16, true);
    let rc = rg
        .prove_dyadic_schedule_closure_v1(&ra, rf, &rs, 1.0e-8, limits)
        .unwrap();
    let ri = rs.evaluate(0.0).unwrap();
    let rr = rs.evaluate(1.0).unwrap();
    let reversed_candidate =
        ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(rs, &ri, &rr).unwrap();
    assert!(
        crate::certify_scheduled_cycle_transition_v1(&rg, &ra, rf, &reversed_candidate, &rc, 32,)
            .is_none()
    );
    for thickness in [0.1, 1.0, 3.0] {
        assert_eq!(
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
            ),
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                &rg,
                &ra,
                rf,
                &reversed_candidate,
                &rc,
                thickness,
                32,
            )
        );
    }
}

#[test]
fn genuine_two_hinge_tree_half_angle_schedule_has_closure_and_bounded_ccd() {
    let points = [
        (0.0, 0.0),
        (33.0, 0.0),
        (66.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (66.0, 100.0),
        (33.0, 100.0),
        (0.0, 100.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("7e00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("7f00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = [fixed_id("7f00", 20), fixed_id("7f00", 21)];
    edges.extend(hinges.iter().enumerate().map(|(index, hinge)| Edge {
        id: *hinge,
        start: boundary[index + 1],
        end: boundary[6 - index],
        kind: if index == 0 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("7a00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("three material faces");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = audit.faces()[0];
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        hinges
            .into_iter()
            .map(|hinge| ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge: hinge,
                u_domain: [
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                    numerator: 10,
                    denominator: 1,
                }],
            })
            .collect(),
        ori_kinematics::CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule, &initial, &requested,
    )
    .unwrap();
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            candidate.schedule(),
            1.0e-9,
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 16,
                max_leaves: 65_536,
                max_work: 1_048_576,
                schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
            },
        )
        .expect("two hinge tree closure");
    let diagnostic =
        diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 8);
    assert_eq!(
        diagnostic.continuous_certificate_model_id(),
        Some(STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1),
    );
    for cancelled_or_excessive in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
        let rejected = diagnose_scheduled_cycle_path_v1(
            &geometry,
            &audit,
            fixed,
            &candidate,
            &closure,
            cancelled_or_excessive,
        );
        assert!(rejected.continuous_certificate_model_id().is_none());
        assert_eq!(rejected.leaf_count(), 0);
        assert_eq!(rejected.pair_work(), 0);
    }
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 8,
        )
        .is_some()
    );
}

#[test]
fn physical_four_vertex_cycle_has_four_radial_hinges_and_only_the_flat_uniform_root() {
    let points = [
        (0.0, 0.0),
        (400.0, 0.0),
        (400.0, 400.0),
        (0.0, 400.0),
        (200.0, 200.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8e00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..4]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let center = vertices[4].id;
    let mut edges = (0..4)
        .map(|index| Edge {
            id: fixed_id("9e00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let crease_edges = (0..4)
        .map(|index| fixed_id("9e00", index as u64 + 10))
        .collect::<Vec<_>>();
    edges.extend((0..4).map(|index| Edge {
        id: crease_edges[index],
        start: boundary[index],
        end: center,
        kind: if index % 2 == 0 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("be00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("four triangular faces");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("cycle geometry");
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(geometry.hinges().len(), 4);
    assert_eq!(audit.closure_hinges().len(), 1);
    let directed_axes = geometry
        .hinges()
        .iter()
        .map(|hinge| {
            (
                hinge.axis().x().to_bits(),
                hinge.axis().y().to_bits(),
                hinge.axis().z().to_bits(),
            )
        })
        .collect::<HashSet<_>>();
    assert_eq!(directed_axes.len(), 4);
    let mut flat = crease_edges
        .iter()
        .map(|edge| HingeAngle::new(*edge, 180.0).unwrap())
        .collect::<Vec<_>>();
    flat.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let flat = CanonicalHingeAngles::new(flat).unwrap();
    assert!(
        geometry
            .solve_closed(&audit, audit.faces()[0], &flat, 1.0e-9)
            .is_ok()
    );
    let moving = vec![crease_edges[1], crease_edges[3]];
    let mut initial = crease_edges
        .iter()
        .map(|edge| {
            HingeAngle::new(*edge, if moving.contains(edge) { 0.0 } else { 180.0 }).unwrap()
        })
        .collect::<Vec<_>>();
    initial.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let initial = CanonicalHingeAngles::new(initial).unwrap();
    assert_eq!(
        enumerate_uniform_cycle_closure_roots_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            &initial,
            &moving,
            180.0,
            128,
        ),
        UniformCycleClosureRootsV1::Roots(vec![180.0])
    );
    let roots = enumerate_uniform_cycle_closure_roots_v1(
        &geometry,
        &audit,
        audit.faces()[0],
        &initial,
        &moving,
        90.0,
        128,
    );
    assert!(matches!(
        roots,
        UniformCycleClosureRootsV1::Indeterminate { .. }
    ));
    let mut reversed = moving.clone();
    reversed.reverse();
    assert_eq!(
        enumerate_uniform_cycle_closure_roots_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            &initial,
            &reversed,
            90.0,
            128,
        ),
        roots
    );
    assert_eq!(
        enumerate_uniform_cycle_closure_roots_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            &initial,
            &reversed,
            90.0,
            1,
        ),
        UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 1 }
    );
    let path = diagnose_collective_cycle_path_v1(
        &geometry,
        &audit,
        audit.faces()[0],
        &initial,
        &moving,
        180.0,
        8,
    );
    assert_eq!(path.continuous_certificate_model_id(), None);
}

#[test]
fn kawasaki_120_120_60_60_vertex_obeys_signed_half_angle_ratio() {
    let points = [
        (100.0, 0.0),
        (-50.0, 86.602_540_378_443_86),
        (-50.0, -86.602_540_378_443_86),
        (50.0, -86.602_540_378_443_86),
        (0.0, 0.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("ae00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..4]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let center = vertices[4].id;
    let hinges = (0..4)
        .map(|index| fixed_id("af00", index as u64 + 10))
        .collect::<Vec<_>>();
    let mut edges = (0..4)
        .map(|index| Edge {
            id: fixed_id("af00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((0..4).map(|index| Edge {
        id: hinges[index],
        start: boundary[index],
        end: center,
        kind: if index == 3 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("aa00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("physical four-vertex topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = audit.faces()[0];
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        hinges
            .iter()
            .enumerate()
            .map(
                |(index, edge)| ori_kinematics::HalfAngleRationalEntryInputV1 {
                    edge: *edge,
                    u_domain: [
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: vec![
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: if index % 2 == 0 { 1 } else { 2 },
                        denominator: 1,
                    }],
                },
            )
            .collect(),
        ori_kinematics::CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    for u in [0.0, 0.5, 1.0] {
        let angles = schedule.evaluate(u).unwrap();
        geometry
            .solve_closed(&audit, fixed, &angles, 1.0e-8)
            .unwrap();
    }
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 16,
                max_leaves: 65_536,
                max_work: 1_048_576,
                schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
            },
        )
        .expect("full-domain physical four-vertex closure");
    assert_eq!(closure.leaves().len(), 1);
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule, &initial, &requested,
    )
    .unwrap();
    let diagnostic =
        diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
    assert!(diagnostic.continuous_certificate_model_id().is_some());
    assert!(
        crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32,
        )
        .is_some()
    );
}

#[test]
fn strict_convex_four_vertex_wedges_bind_every_input_and_fail_closed() {
    let points = [
        (100.0, 100.0),
        (-100.0, 100.0),
        (-100.0, -100.0),
        (100.0, -100.0),
        (0.0, 0.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("de00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..4].iter().map(|v| v.id).collect::<Vec<_>>();
    let center = vertices[4].id;
    let hinges = (0..4)
        .map(|index| fixed_id("df00", index as u64 + 10))
        .collect::<Vec<_>>();
    let mut edges = (0..4)
        .map(|index| Edge {
            id: fixed_id("df00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((0..4).map(|index| Edge {
        id: hinges[index],
        start: boundary[index],
        end: center,
        kind: if index == 3 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("dc00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = audit.faces()[0];
    let schedule_limits = ori_kinematics::CycleScheduleLimitsV1::default();
    let closure_limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
        max_depth: 2,
        max_leaves: 4,
        max_work: 1_000_000,
        schedule_limits,
    };
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        hinges
            .iter()
            .map(|edge| ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    ori_kinematics::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                }],
                denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, closure_limits)
        .expect("constant strict-convex cardinal schedule closes exactly");
    let max_leaf_work = 16_777_216;
    let binding = || DyadicFaceTransformBindingInputV1 {
        geometry: &geometry,
        audit: &audit,
        fixed_face: fixed,
        schedule: &schedule,
        closure: &closure,
        thickness_mm: 0.1,
        tolerance: 1.0e-8,
        schedule_limits,
        max_work_per_leaf: max_leaf_work,
    };
    let transforms = prepare_dyadic_face_transform_interval_registry_v1(
        &geometry,
        &audit,
        fixed,
        &schedule,
        &closure,
        0.1,
        1.0e-8,
        schedule_limits,
        max_leaf_work,
    )
    .unwrap();
    let pairs = diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule).unwrap();
    assert!(
        pairs.entries().iter().any(|entry| {
            entry.kind() == ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor
        })
    );
    let gaps = diagnose_shared_vertex_continuous_corridor_gaps_v1(
        &pairs, &geometry, &audit, fixed, &schedule, 0.1,
    )
    .unwrap();
    assert!(!gaps.gaps().is_empty());
    let mut records = gaps
        .gaps()
        .iter()
        .map(|gap| {
            let mut incident_faces = geometry
                .face_ids()
                .iter()
                .copied()
                .filter(|face| {
                    geometry
                        .face_boundary_vertices(*face)
                        .is_some_and(|vertices| vertices.contains(&gap.vertex()))
                })
                .collect::<Vec<_>>();
            incident_faces.sort_unstable_by_key(FaceId::canonical_bytes);
            crate::VertexReliefPolicyRecordV1 {
                vertex: gap.vertex(),
                cutout_radius_mm: 0.1,
                material_thickness_mm: 0.1,
                incident_faces,
            }
        })
        .collect::<Vec<_>>();
    records.sort_unstable_by_key(|record| record.vertex.canonical_bytes());
    records.dedup_by_key(|record| record.vertex);
    assert!(!records.is_empty());
    let relief = crate::prepare_vertex_relief_prerequisite_v1(&geometry, 0.1, &records).unwrap();
    let sectors = diagnose_dyadic_shared_vertex_sector_boundaries_v1(
        &transforms,
        &gaps,
        &relief,
        &records,
        binding(),
        max_leaf_work,
    )
    .unwrap();
    let cell_work = 1_000_000;
    let wedges = diagnose_dyadic_shared_vertex_wedges_v1(
        &sectors,
        &transforms,
        &gaps,
        &relief,
        &records,
        binding(),
        cell_work,
    )
    .unwrap();
    assert!(!wedges.authorizes_continuous_motion());
    assert!(!wedges.authorizes_project_mutation());
    assert!(wedges.is_for(
        &sectors,
        &transforms,
        &gaps,
        &relief,
        &records,
        binding(),
        cell_work,
    ));
    assert!(wedges.leaves().iter().all(|(_, _, cells)| {
        !cells.is_empty()
            && cells.iter().all(|cell| {
                !cell.top_ring().is_empty() && cell.top_ring().len() == cell.bottom_ring().len()
            })
    }));

    macro_rules! rejects_wedge {
        ($change:expr) => {{
            let mut foreign = wedges.clone();
            $change(&mut foreign);
            assert!(!foreign.is_for(
                &sectors,
                &transforms,
                &gaps,
                &relief,
                &records,
                binding(),
                cell_work,
            ));
        }};
    }
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.schedule_hash[0] ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.closure_hash[0] ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.thickness_bits ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.max_work_per_cell -= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.radius_binding[0].1 ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.sector_content_hash[0] ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.content_hash[0] ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].0 ^= 1);
    rejects_wedge!(|w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].1 ^= 1);
    rejects_wedge!(
        |w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].2[0].pair[0] =
            fixed_id("dd00", 1)
    );
    rejects_wedge!(
        |w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].2[0].vertex = fixed_id("dd00", 2)
    );
    rejects_wedge!(
        |w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].2[0].face = fixed_id("dd00", 3)
    );
    rejects_wedge!(
        |w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].2[0].top_ring[0][0] =
            ori_kinematics::OutwardIntervalV1::new(0.0, 0.0).unwrap()
    );
    rejects_wedge!(
        |w: &mut DyadicSharedVertexWedgeDiagnosticV1| w.leaves[0].2[0].bottom_ring[0][0] =
            ori_kinematics::OutwardIntervalV1::new(0.0, 0.0).unwrap()
    );
    assert!(!wedges.is_for(
        &sectors,
        &transforms,
        &gaps,
        &relief,
        &records,
        DyadicFaceTransformBindingInputV1 {
            thickness_mm: 0.2,
            ..binding()
        },
        cell_work,
    ));
    let foreign_geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    assert!(!wedges.is_for(
        &sectors,
        &transforms,
        &gaps,
        &relief,
        &records,
        DyadicFaceTransformBindingInputV1 {
            geometry: &foreign_geometry,
            ..binding()
        },
        cell_work,
    ));
    assert!(matches!(
        diagnose_dyadic_shared_vertex_wedges_v1(
            &sectors,
            &transforms,
            &gaps,
            &relief,
            &records,
            binding(),
            0,
        ),
        Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
    ));
    assert!(matches!(
        diagnose_dyadic_shared_vertex_wedges_v1(
            &sectors,
            &transforms,
            &gaps,
            &relief,
            &records,
            binding(),
            MAX_SHARED_VERTEX_WEDGE_WORK_V1 + 1,
        ),
        Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
    ));
    assert!(matches!(
        diagnose_dyadic_shared_vertex_wedges_v1(
            &sectors,
            &transforms,
            &gaps,
            &relief,
            &records,
            binding(),
            1,
        ),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    ));
    // The two opposite face pairs are separated on a world coordinate
    // axis for the complete transformed prisms, not just ring samples.
    let separation = diagnose_dyadic_shared_vertex_wedge_separation_v1(
        &wedges,
        binding(),
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    )
    .unwrap();
    assert!(!separation.authorizes_continuous_motion());
    assert!(!separation.authorizes_project_mutation());
    assert!(separation.is_for(
        &wedges,
        binding(),
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    ));
    assert!(
        separation
            .leaves()
            .iter()
            .all(|(_, _, bounds)| !bounds.is_empty()
                && bounds.iter().all(|bound| bound.lower_mm() > 0.0))
    );
    let mut tampered = separation.clone();
    tampered.leaves[0].2.pop();
    assert!(!tampered.is_for(
        &wedges,
        binding(),
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    ));
    let mut tampered = separation.clone();
    let duplicate = tampered.leaves[0].2[0];
    tampered.leaves[0].2.push(duplicate);
    assert!(!tampered.is_for(
        &wedges,
        binding(),
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    ));
    macro_rules! rejects_separation {
        ($change:expr) => {{
            let mut foreign = separation.clone();
            $change(&mut foreign);
            assert!(!foreign.is_for(
                &wedges,
                binding(),
                MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
            ));
        }};
    }
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.schedule_hash[0] ^= 1
    );
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.closure_hash[0] ^= 1
    );
    rejects_separation!(|d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d
        .thickness_bits ^=
        1);
    rejects_separation!(|d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d
        .max_work_per_pair -=
        1);
    rejects_separation!(|d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d
        .wedge_content_hash[0] ^=
        1);
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.content_hash[0] ^= 1
    );
    rejects_separation!(|d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.leaves[0].0 ^= 1);
    rejects_separation!(|d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.leaves[0].1 ^= 1);
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.leaves[0].2[0].lower_mm =
            f64::NAN
    );
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.leaves[0].2[0].pair[0] =
            fixed_id("dd00", 4)
    );
    rejects_separation!(
        |d: &mut DyadicSharedVertexWedgeSeparationDiagnosticV1| d.leaves[0].2[0].vertex =
            fixed_id("dd00", 5)
    );
    for change in 0..4 {
        let mut foreign_wedges = wedges.clone();
        match change {
            0 => foreign_wedges.schedule_hash[0] ^= 1,
            1 => foreign_wedges.closure_hash[0] ^= 1,
            2 => foreign_wedges.thickness_bits ^= 1,
            _ => foreign_wedges.content_hash[0] ^= 1,
        }
        assert!(!separation.is_for(
            &foreign_wedges,
            binding(),
            MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
        ));
    }
    assert!(!separation.is_for(
        &wedges,
        DyadicFaceTransformBindingInputV1 {
            thickness_mm: 0.2,
            ..binding()
        },
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    ));
    let mut foreign_wedges = wedges.clone();
    foreign_wedges.issuer = foreign_geometry.instance_anchor_v1();
    assert!(!separation.is_for(
        &foreign_wedges,
        binding(),
        MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1,
    ));
    for invalid in [0, MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1 + 1] {
        assert!(matches!(
            diagnose_dyadic_shared_vertex_wedge_separation_v1(&wedges, binding(), invalid),
            Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
        ));
    }
    assert!(matches!(
        diagnose_dyadic_shared_vertex_wedge_separation_v1(&wedges, binding(), 1),
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    ));
}

#[test]
fn certified_cardinal_degree_four_remains_unsupported_without_vertex_relief() {
    let points = [
        (100.0, 100.0),
        (-100.0, 100.0),
        (-100.0, -100.0),
        (100.0, -100.0),
        (0.0, 0.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("ce00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..4]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let center = vertices[4].id;
    let hinges = (0..4)
        .map(|index| fixed_id("cf00", index as u64 + 10))
        .collect::<Vec<_>>();
    let mut edges = (0..4)
        .map(|index| Edge {
            id: fixed_id("cf00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((0..4).map(|index| Edge {
        id: hinges[index],
        start: boundary[index],
        end: center,
        kind: if index == 3 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let project = fixed_id("cc00", 1);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .unwrap();
    let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
    let global = ori_foldability::analyze_global_flat_foldability(
        ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
            project, &paper, &pattern, &topology, &local,
        ),
        ori_foldability::GlobalFlatFoldabilityLimits::default(),
    )
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let selected = [(hinges[0], false), (hinges[1], false), (hinges[2], false)];
    let completion_candidates =
        crate::general_cell_transport::degree_four_petal_completion_candidates_v1(
            &geometry, selected,
        );
    assert_eq!(completion_candidates.len(), 4);
    assert!(
        completion_candidates.iter().any(|candidate| {
            crate::prepare_regular_quad_petal_schedules_v1(
                &geometry,
                &audit,
                audit.faces()[0],
                candidate,
                ori_kinematics::CycleScheduleLimitsV1::default(),
            )
            .is_some_and(|schedules| {
                schedules.iter().all(|schedule| {
                    geometry
                        .solve_closed(
                            &audit,
                            audit.faces()[0],
                            &schedule.evaluate(1.0).unwrap(),
                            1.0e-9,
                        )
                        .is_ok()
                })
            })
        }),
        "no bounded endpoint closes"
    );
    let authority = crate::issue_regular_quad_petal_chained_authority_v1(
        &geometry,
        &audit,
        global
            .layer_order()
            .expect("Kawasaki and Maekawa authority"),
        audit.faces()[0],
        selected,
        0.1,
        1.0e-9,
        ori_kinematics::CycleScheduleLimitsV1::default(),
        ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_depth: 8,
            max_leaves: 256,
            max_work: 1_000_000,
            schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
        },
    );
    assert!(
        authority.is_none(),
        "positive thickness needs vertex relief"
    );
}

fn one_hinge_model() -> MaterialTreeKinematicsModel {
    let points = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8100", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9100", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fixed_id("9100", 6),
        start: boundary[0],
        end: boundary[2],
        kind: EdgeKind::Mountain,
    });
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let project: ProjectId = fixed_id("b100", 1);
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap()
}

fn two_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (450.0, 200.0),
        (250.0, 450.0),
        (0.0, 300.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8200", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9200", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("9200", 6),
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("9200", 7),
            start: boundary[0],
            end: boundary[3],
            kind: EdgeKind::Valley,
        },
    ]);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b200", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("three triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("two-hinge triangle model")
}

fn three_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (500.0, 150.0),
        (500.0, 400.0),
        (250.0, 550.0),
        (0.0, 300.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8500", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9500", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9500", 10 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b500", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("four triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("three-hinge triangular tree")
}

fn four_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (520.0, 120.0),
        (620.0, 350.0),
        (480.0, 580.0),
        (200.0, 650.0),
        (0.0, 320.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8600", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9600", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9600", 10 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b600", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("five triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("four-hinge triangular tree")
}

fn five_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (520.0, 90.0),
        (680.0, 280.0),
        (650.0, 500.0),
        (450.0, 680.0),
        (180.0, 700.0),
        (0.0, 340.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8700", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9700", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5, 6].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9700", 10 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b700", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("six triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("five-hinge triangular tree")
}

fn six_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (530.0, 70.0),
        (700.0, 220.0),
        (760.0, 430.0),
        (620.0, 640.0),
        (380.0, 760.0),
        (140.0, 720.0),
        (0.0, 360.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8800", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9800", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5, 6, 7].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9800", 10 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b800", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("seven triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("six-hinge triangular tree")
}

fn seven_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (540.0, 60.0),
        (730.0, 190.0),
        (840.0, 380.0),
        (810.0, 580.0),
        (650.0, 760.0),
        (410.0, 850.0),
        (150.0, 780.0),
        (0.0, 390.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8900", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9900", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5, 6, 7, 8].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9900", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b900", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("eight triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("seven-hinge triangular tree")
}

fn eight_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (540.0, 60.0),
        (730.0, 190.0),
        (840.0, 380.0),
        (850.0, 570.0),
        (760.0, 750.0),
        (590.0, 880.0),
        (370.0, 930.0),
        (150.0, 850.0),
        (0.0, 430.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8a00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9a00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5, 6, 7, 8, 9].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9a00", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("ba00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("nine triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("eight-hinge triangular tree")
}

fn nine_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (530.0, 45.0),
        (720.0, 140.0),
        (850.0, 300.0),
        (900.0, 490.0),
        (860.0, 680.0),
        (730.0, 840.0),
        (530.0, 940.0),
        (310.0, 960.0),
        (120.0, 850.0),
        (0.0, 460.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8c00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9c00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in [2, 3, 4, 5, 6, 7, 8, 9, 10].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9c00", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bc00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("ten triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("nine-hinge triangular tree")
}

fn ten_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (520.0, 35.0),
        (710.0, 110.0),
        (860.0, 240.0),
        (940.0, 410.0),
        (950.0, 590.0),
        (880.0, 760.0),
        (740.0, 900.0),
        (550.0, 980.0),
        (340.0, 990.0),
        (140.0, 880.0),
        (0.0, 480.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8d00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9d00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=11).enumerate() {
        edges.push(Edge {
            id: fixed_id("9d00", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bd00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("eleven triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("ten-hinge triangular tree")
}

fn eleven_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (520.0, 35.0),
        (710.0, 110.0),
        (860.0, 240.0),
        (940.0, 410.0),
        (950.0, 590.0),
        (880.0, 760.0),
        (740.0, 900.0),
        (550.0, 980.0),
        (340.0, 990.0),
        (140.0, 880.0),
        (60.0, 700.0),
        (0.0, 480.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8e00", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9e00", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=12).enumerate() {
        edges.push(Edge {
            id: fixed_id("9e00", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("be00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("twelve triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("eleven-hinge triangular tree")
}

fn twelve_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0., 0.),
        (300., 0.),
        (500., 20.),
        (680., 65.),
        (840., 145.),
        (970., 265.),
        (1050., 420.),
        (1070., 580.),
        (1030., 735.),
        (930., 870.),
        (780., 970.),
        (590., 1025.),
        (390., 1025.),
        (180., 930.),
        (0., 520.),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Vertex {
            id: fixed_id("8f00", i as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|i| Edge {
            id: fixed_id("9f00", i as u64 + 1),
            start: boundary[i],
            end: boundary[(i + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=13).enumerate() {
        edges.push(Edge {
            id: fixed_id("9f00", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bf00", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("thirteen triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("twelve-hinge triangular tree")
}

fn thirteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0., 0.),
        (4., 0.),
        (7., 1.),
        (9., 3.),
        (11., 6.),
        (12., 9.),
        (12., 12.),
        (11., 15.),
        (9., 18.),
        (7., 20.),
        (4., 21.),
        (2., 20.),
        (1., 18.),
        (0., 15.),
        (-1., 11.),
        (-1., 6.),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Vertex {
            id: fixed_id("8a10", i as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|i| Edge {
            id: fixed_id("9a10", i as u64 + 1),
            start: boundary[i],
            end: boundary[(i + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=14).enumerate() {
        edges.push(Edge {
            id: fixed_id("9a10", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("ba10", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("fourteen triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("thirteen-hinge triangular tree")
}

fn fourteen_hinge_triangle_model_with_leaf_offset(
    leaf_x_offset: f64,
) -> MaterialTreeKinematicsModel {
    let points = [
        (0., 0.),
        (4. + leaf_x_offset, 0.),
        (7., 1.),
        (10., 3.),
        (12., 6.),
        (13., 9.),
        (13., 12.),
        (12., 15.),
        (10., 18.),
        (8., 20.),
        (5., 22.),
        (3., 22.),
        (1., 20.),
        (0., 18.),
        (-1., 15.),
        (-2., 10.),
        (-1., 4.),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Vertex {
            id: fixed_id("8b10", i as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|i| Edge {
            id: fixed_id("9b10", i as u64 + 1),
            start: boundary[i],
            end: boundary[(i + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=15).enumerate() {
        edges.push(Edge {
            id: fixed_id("9b10", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bb10", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("fifteen triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("fourteen-hinge triangular tree")
}

fn fourteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    fourteen_hinge_triangle_model_with_leaf_offset(0.0)
}

fn fifteen_hinge_triangle_model_with_edge_order(
    reverse_edges: bool,
) -> MaterialTreeKinematicsModel {
    let points = [
        (0., 0.),
        (4., 0.),
        (8., 1.),
        (11., 3.),
        (14., 6.),
        (16., 10.),
        (17., 14.),
        (17., 18.),
        (16., 22.),
        (14., 26.),
        (11., 29.),
        (8., 31.),
        (5., 32.),
        (3., 31.),
        (1., 29.),
        (0., 26.),
        (-1., 20.),
        (-1., 10.),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Vertex {
            id: fixed_id("8c10", i as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|i| Edge {
            id: fixed_id("9c10", i as u64 + 1),
            start: boundary[i],
            end: boundary[(i + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (offset, end) in (2..=16).enumerate() {
        edges.push(Edge {
            id: fixed_id("9c10", 20 + offset as u64),
            start: boundary[0],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    if reverse_edges {
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bc10", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("sixteen triangles"),
        TreeKinematicsLimits::default(),
    )
    .expect("fifteen-hinge triangular tree")
}

fn fifteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
    fifteen_hinge_triangle_model_with_edge_order(false)
}

fn branched_triangle_model(face_count: usize, reverse_edges: bool) -> MaterialTreeKinematicsModel {
    let vertex_count = face_count + 2;
    let vertices = (0..vertex_count)
        .map(|index| Vertex {
            id: fixed_id("8d10", index as u64 + 1),
            position: Point2::new(index as f64, (index * index) as f64),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..vertex_count)
        .map(|index| Edge {
            id: fixed_id("9d10", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % vertex_count],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let first_branch = vertex_count / 3;
    let second_branch = vertex_count * 2 / 3;
    let mut diagonals = vec![
        (0, first_branch),
        (first_branch, second_branch),
        (second_branch, 0),
    ];
    diagonals.extend((2..first_branch).map(|end| (0, end)));
    diagonals.extend((first_branch + 2..second_branch).map(|end| (first_branch, end)));
    diagonals.extend((second_branch + 2..vertex_count).map(|end| (second_branch, end)));
    diagonals.sort_unstable();
    diagonals.dedup();
    for (offset, (start, end)) in diagonals.into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("9d10", 30 + offset as u64),
            start: boundary[start],
            end: boundary[end],
            kind: if offset % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    if reverse_edges {
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bd10", face_count as u64),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("branched triangulation"),
        TreeKinematicsLimits::default(),
    )
    .expect("branched triangular tree")
}

fn zero_tree_pose(
    model: &MaterialTreeKinematicsModel,
) -> (Vec<EdgeId>, ori_kinematics::MaterialTreePose) {
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    (moving, pose)
}

fn two_hinge_strip_model() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (1.0, 0.0),
        (3.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (3.0, 4.0),
        (1.0, 4.0),
        (0.0, 4.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8200", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9200", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("9200", 20),
            start: boundary[1],
            end: boundary[6],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("9200", 21),
            start: boundary[2],
            end: boundary[5],
            kind: EdgeKind::Mountain,
        },
    ]);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b200", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap()
}

fn three_hinge_strip_model(narrow_gap: bool) -> MaterialTreeKinematicsModel {
    let middle = if narrow_gap { 2.01 } else { 3.0 };
    let points = [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (middle, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (middle, 4.0),
        (2.0, 4.0),
        (1.0, 4.0),
        (0.0, 4.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8300", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9300", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend(
        [(1, 8), (2, 7), (3, 6)]
            .into_iter()
            .enumerate()
            .map(|(index, (start, end))| Edge {
                id: fixed_id("9300", 20 + index as u64),
                start: boundary[start],
                end: boundary[end],
                kind: EdgeKind::Mountain,
            }),
    );
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b300", if narrow_gap { 2 } else { 1 }),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap()
}

fn deep_strip_model(hinge_count: usize) -> MaterialTreeKinematicsModel {
    let column_count = hinge_count + 2;
    let mut points = (0..column_count)
        .map(|column| (column as f64 * 100.0, 0.0))
        .collect::<Vec<_>>();
    points.extend(
        (0..column_count)
            .rev()
            .map(|column| (column as f64 * 100.0, 4.0)),
    );
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8400", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9400", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((1..=hinge_count).map(|column| Edge {
        id: fixed_id("9400", 1_000 + column as u64),
        start: boundary[column],
        end: boundary[2 * column_count - 1 - column],
        kind: EdgeKind::Mountain,
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b400", hinge_count as u64),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap()
}

fn sparse_triangle_strip_model(face_count: usize) -> MaterialTreeKinematicsModel {
    assert!((3..=64).contains(&face_count));
    let cell_count = face_count.div_ceil(2);
    let first_bottom = usize::from(face_count % 2 == 1);
    let mut points = (first_bottom..=cell_count)
        .map(|column| (column as f64 * 100.0, 0.0))
        .collect::<Vec<_>>();
    points.extend(
        (0..=cell_count)
            .rev()
            .map(|column| (column as f64 * 100.0, 4.0)),
    );
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("8f20", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut bottom = vec![None; cell_count + 1];
    let mut top = vec![None; cell_count + 1];
    for vertex in &vertices {
        let column = (vertex.position.x / 100.0) as usize;
        if vertex.position.y == 0.0 {
            bottom[column] = Some(vertex.id);
        } else {
            top[column] = Some(vertex.id);
        }
    }
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("9f20", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut next_edge_id = boundary.len() as u64 + 1;
    for column in 1..cell_count {
        edges.push(Edge {
            id: fixed_id("9f20", next_edge_id),
            start: bottom[column].unwrap(),
            end: top[column].unwrap(),
            kind: EdgeKind::Mountain,
        });
        next_edge_id += 1;
    }
    for column in first_bottom..cell_count {
        edges.push(Edge {
            id: fixed_id("9f20", next_edge_id),
            start: bottom[column].unwrap(),
            end: top[column + 1].unwrap(),
            kind: EdgeKind::Valley,
        });
        next_edge_id += 1;
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("bf20", face_count as u64),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("sparse triangle strip"),
        TreeKinematicsLimits::default(),
    )
    .expect("sparse triangular tree")
}

#[test]
fn nine_hinge_deep_tree_is_certified_deterministically_across_input_permutation() {
    let model = deep_strip_model(9);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let zero_candidate = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        0.01,
        0.0,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
    )
    .unwrap();
    assert!(zero_candidate.continuous_clearance_certified());
    assert_eq!(zero_candidate.interval_pair_work(), 0);
    let first = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        5.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
    )
    .unwrap();
    let mut reversed = moving;
    reversed.reverse();
    let second = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &reversed,
        5.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
    )
    .unwrap();
    assert!(first.continuous_clearance_certified());
    assert_eq!(first, second);
    assert!(first.interval_leaf_count() >= 1);
    assert!(first.interval_pair_work() > 0);
}

#[test]
fn sixteen_hinge_overlap_exhausts_adaptive_budget_fail_closed() {
    let model = sparse_triangle_strip_model(17);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let result = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        180.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
    )
    .unwrap();
    assert!(!result.continuous_clearance_certified());
    assert_eq!(result.interval_leaf_count(), 0);
    assert_eq!(result.interval_pair_work(), 0);
}

#[test]
fn twenty_four_hinge_sparse_tree_uses_complete_sweep_candidates() {
    let model = deep_strip_model(24);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let mut metrics = (0, 0);
    assert!(two_hinge_interval_clearance_premises(
        &model,
        &pose,
        &moving,
        0.001,
        1,
        &mut metrics,
        &CooperativeOperationControlV1::unbounded(),
    ));
    assert_eq!(metrics.0, 1);
    assert!(metrics.1 < MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1);
}

#[test]
fn thirty_two_hinge_dense_tree_exceeds_candidate_cap() {
    let model = deep_strip_model(32);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let mut metrics = (0, 0);
    assert!(!two_hinge_interval_clearance_premises(
        &model,
        &pose,
        &moving,
        180.0,
        1,
        &mut metrics,
        &CooperativeOperationControlV1::unbounded(),
    ));
    assert_eq!(metrics, (0, 0));
}

#[test]
fn forty_eight_hinge_sparse_tree_uses_one_canonical_candidate_scan() {
    let model = deep_strip_model(48);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let mut metrics = (0, 0);
    assert!(two_hinge_interval_clearance_premises(
        &model,
        &pose,
        &moving,
        0.0001,
        1,
        &mut metrics,
        &CooperativeOperationControlV1::unbounded(),
    ));
    assert_eq!(metrics.0, 1);
    assert!(metrics.1 <= MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1);
}

#[test]
fn sixty_four_hinge_dense_tree_fails_candidate_cap() {
    let model = deep_strip_model(64);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let mut metrics = (0, 0);
    assert!(!two_hinge_interval_clearance_premises(
        &model,
        &pose,
        &moving,
        180.0,
        1,
        &mut metrics,
        &CooperativeOperationControlV1::unbounded(),
    ));
    assert_eq!(metrics, (0, 0));
}

#[test]
fn authenticated_two_face_zero_thickness_path_gets_narrow_certificate() {
    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let result = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &[edge],
        90.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(result.continuous_clearance_certified());
    assert_eq!(
        result.continuous_certificate_model_id(),
        Some(STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
    );
    assert_eq!(result.safe_stop_angle_degrees(), 90.0);

    let positive_thickness = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &[edge],
        37.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("positive-thickness path");
    assert!(positive_thickness.continuous_clearance_certified());
    assert_eq!(
        positive_thickness.continuous_certificate_model_id(),
        Some(STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2)
    );
    assert_ne!(
        positive_thickness.continuous_certificate_model_id(),
        Some("stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1")
    );
    assert_eq!(positive_thickness.safe_stop_angle_degrees(), 37.0);

    let requested =
        CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 37.0).expect("requested hinge")])
            .expect("canonical requested hinge");
    let certificate =
        certify_positive_thickness_tree_continuous_path_v1(&model, &pose, &requested, 0.1)
            .expect("deterministic single-hinge certificate");
    assert_eq!(
        certificate.binding_fingerprint_v1(),
        [
            0x11, 0xe3, 0xe4, 0xf5, 0x27, 0xdf, 0xa7, 0x1e, 0x25, 0xed, 0x60, 0x90, 0x83, 0x7c,
            0x82, 0x47, 0x46, 0x35, 0xb8, 0x07, 0x08, 0xd0, 0x1e, 0xf1, 0xb7, 0x46, 0x82, 0x46,
            0x45, 0xb6, 0xf1, 0xea,
        ],
        "the V2 outward model and deterministic transcendental model are part of the binding"
    );
    let first_pose = model
        .solve(Some(model.face_ids()[0]), &requested)
        .expect("first requested pose");
    let equal_but_distinct_pose = model
        .solve(Some(model.face_ids()[0]), &requested)
        .expect("ABA requested pose");
    let first_bound = model.bind_pose(&first_pose).expect("first bound");
    let boundary = prepare_single_hinge_thickness_boundary_v1(first_bound, 0.1)
        .expect("bounded classification")
        .expect("positive-thickness outer shell");
    assert!(
        revalidate_single_hinge_thickness_boundary_v1(
            &boundary,
            model
                .bind_pose(&equal_but_distinct_pose)
                .expect("distinct bound"),
            0.1,
        )
        .is_none()
    );
    assert!(
        revalidate_single_hinge_thickness_boundary_v1(
            &boundary,
            first_bound,
            f64::from_bits(0.1_f64.to_bits() + 1),
        )
        .is_none()
    );
}

#[test]
fn sampled_layer_callback_receives_retained_initial_pose_instance_at_zero_sample() {
    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let source = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &source)
        .expect("initial single-hinge pose");
    let target = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 180.0).unwrap()]).unwrap();
    let zero_sample_seen = std::cell::Cell::new(false);
    let retained_initial_only =
        |index: usize, pose: &MaterialTreePose, _: &StaticCollisionDiagnosticSnapshot| {
            if index == 0 {
                zero_sample_seen.set(true);
                pose.same_instance(&initial)
            } else {
                false
            }
        };

    diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
        &model,
        &initial,
        initial.hinge_angles(),
        target.as_slice(),
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
        None,
        Some(&retained_initial_only),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("the source-bound callback must accept the retained zero sample");

    assert!(zero_sample_seen.get());
}

#[test]
fn sampled_layer_callback_rejection_cannot_fall_through_analytic_bypasses() {
    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let source = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &source)
        .expect("initial single-hinge pose");
    let target = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 180.0).unwrap()]).unwrap();
    let initial_sample_only =
        |index: usize, _: &MaterialTreePose, _: &StaticCollisionDiagnosticSnapshot| index == 0;
    let limits = StackedFoldPathDiagnosticLimitsV1::default();
    let diagnostic = diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
        &model,
        &initial,
        initial.hinge_angles(),
        target.as_slice(),
        0.0,
        limits,
        None,
        Some(&initial_sample_only),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("sampled-layer diagnostic");

    assert_eq!(
        diagnostic
            .first_sampled_blocking_angle_degrees()
            .map(f64::to_bits),
        Some(180.0_f64.to_bits())
    );
    assert_eq!(
        diagnostic.sampled_nonblocking_pose_count(),
        diagnostic.sampled_pose_count() - 1
    );
    assert!(!diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.continuous_certificate_model_id(), None);
}

#[test]
fn nondirect_initial_only_flat_pair_blocks_the_first_positive_sample() {
    // `SharedFeatureFlatStack` currently implies one direct hinge in every
    // valid material Tree, so a production geometry with no direct hinge
    // cannot be constructed. Exercise that defensive initial-only branch
    // without forging geometry: retain one real exact flat-pair identity,
    // then remove only its direct-hinge observation at the pure admission
    // decision boundary. This protects future broader static evidence.
    let model = two_hinge_triangle_model();
    let stationary = &model.hinges()[0];
    let moving = model.hinges()[1].edge();
    let source = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if hinge.edge() == stationary.edge() {
                        180.0
                    } else {
                        0.0
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("valid defensive source angles"),
    )
    .expect("canonical defensive source angles");
    let initial = model
        .solve(Some(model.face_ids()[0]), &source)
        .expect("defensive initial Tree pose");
    let target = CanonicalHingeAngles::new(
        source
            .as_slice()
            .iter()
            .map(|angle| {
                HingeAngle::new(
                    angle.edge(),
                    if angle.edge() == moving {
                        37.0
                    } else {
                        angle.angle_degrees()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("valid defensive target angles"),
    )
    .expect("canonical defensive target angles");
    let mut pair = (stationary.left_face(), stationary.right_face());
    if pair.0.canonical_bytes() > pair.1.canonical_bytes() {
        pair = (pair.1, pair.0);
    }
    let initial_only = initial_sample_layer_admission::classify_initial_layer_pair_admission_v1(
        pair,
        true,
        crate::StaticCollisionPairDisposition::Indeterminate,
        crate::IntersectionEvidenceV2::SharedFeatureFlatStack,
        None,
        None,
    );
    assert_eq!(initial_only.pair, pair);
    assert_eq!(
        initial_only.kind,
        initial_sample_layer_admission::InitialLayerPairAdmissionKindV1::InitialOnlyFlatStack
    );

    let positive_missing_direct_seen = std::cell::Cell::new(false);
    let initial_only_matcher =
        |index: usize, _: &MaterialTreePose, snapshot: &StaticCollisionDiagnosticSnapshot| {
            let observed = snapshot
                .pairs()
                .iter()
                .find(|candidate| {
                    let mut candidate_pair = (candidate.first_face(), candidate.second_face());
                    if candidate_pair.0.canonical_bytes() > candidate_pair.1.canonical_bytes() {
                        candidate_pair = (candidate_pair.1, candidate_pair.0);
                    }
                    candidate_pair == pair
                })
                .expect("stationary exact flat pair identity");
            assert_eq!(
                observed.evidence(),
                crate::IntersectionEvidenceV2::SharedFeatureFlatStack
            );
            assert_eq!(
                observed.disposition(),
                crate::StaticCollisionPairDisposition::Indeterminate
            );
            if index == 0 {
                return initial_only.kind
                == initial_sample_layer_admission::InitialLayerPairAdmissionKindV1::InitialOnlyFlatStack;
            }
            let rejection =
                initial_sample_layer_admission::diagnose_nondirect_positive_flat_stack_for_test_v1(
                    pair,
                )
                .expect_err("positive nondirect flat pair must not be admitted");
            assert_eq!(rejection.pair, pair);
            assert_eq!(
            rejection.reason,
            initial_sample_layer_admission::PersistentFlatStackSampleRejectionReasonV1::MissingDirectSharedHinge
        );
            positive_missing_direct_seen.set(true);
            false
        };
    let limits = StackedFoldPathDiagnosticLimitsV1::default();
    let diagnostic = diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
        &model,
        &initial,
        initial.hinge_angles(),
        target.as_slice(),
        0.0,
        limits,
        None,
        Some(&initial_only_matcher),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("defensive initial-only path diagnostic");

    assert!(positive_missing_direct_seen.get());
    assert_eq!(
        diagnostic
            .first_sampled_blocking_angle_degrees()
            .map(f64::to_bits),
        Some((37.0 / limits.sample_intervals as f64).to_bits())
    );
    assert_eq!(diagnostic.sampled_nonblocking_pose_count(), 1);
    assert!(!diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.continuous_certificate_model_id(), None);
}

#[test]
fn three_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = two_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    for requested in [10.0, 30.0, 45.0, 60.0] {
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(
            diagnostic.continuous_certificate_model_id(),
            Some(STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2)
        );
        assert_ne!(
            diagnostic.continuous_certificate_model_id(),
            Some("stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1")
        );
        assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
    }
}

#[test]
fn four_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = three_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    for requested in [10.0, 30.0, 60.0] {
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified(), "{requested}");
        assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
    }
}

#[test]
fn eight_triangle_positive_thickness_tree_rejects_over_angle() {
    let model = seven_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let beyond_bound = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        15.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded hold");
    assert!(!beyond_bound.continuous_clearance_certified());
    assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
}

#[test]
fn five_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = four_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    for requested in [10.0, 30.0, 45.0] {
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified(), "{requested}");
        assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
        let target = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, requested).unwrap())
                .collect(),
        )
        .unwrap();
        let certificate =
            certify_positive_thickness_tree_continuous_path_v1(&model, &initial, &target, 0.1)
                .expect("issuer-bound four-hinge certificate");
        assert!(certificate.is_for(&model, &initial, &target, 0.1));
        assert!(!certificate.authorizes_project_mutation());
        assert!(!certificate.is_for(
            &model,
            &initial,
            &target,
            f64::from_bits(0.1_f64.to_bits() + 1),
        ));
    }
    let nonzero_source = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 10.0).unwrap())
            .collect(),
    )
    .unwrap();
    let nonzero_pose = model
        .solve(Some(model.face_ids()[0]), &nonzero_source)
        .unwrap();
    let nonzero_target = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 20.0).unwrap())
            .collect(),
    )
    .unwrap();
    let certificate = certify_positive_thickness_tree_continuous_path_v1(
        &model,
        &nonzero_pose,
        &nonzero_target,
        0.1,
    )
    .expect("positive Tree proof is bounded by absolute-pose excursion");
    assert!(certificate.is_for(&model, &nonzero_pose, &nonzero_target, 0.1));
    let beyond_bound = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        45.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded hold");
    assert!(!beyond_bound.continuous_clearance_certified());
    assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
}

#[test]
fn six_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = five_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let requested = 30.0;
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        requested,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded positive-thickness diagnosis");
    assert!(diagnostic.continuous_clearance_certified(), "{requested}");
    assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
    let beyond_bound = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        30.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded hold");
    assert!(!beyond_bound.continuous_clearance_certified());
    assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
}

#[test]
fn seven_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = six_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        20.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded positive-thickness diagnosis");
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 20.0);
    let beyond_bound = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        20.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded hold");
    assert!(!beyond_bound.continuous_clearance_certified());
    assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
}

#[test]
fn eight_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = seven_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        15.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded positive-thickness diagnosis");
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 15.0);
}

#[test]
fn nine_triangle_positive_thickness_tree_rejects_over_angle() {
    let model = eight_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        10.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded positive-thickness diagnosis");
    assert!(!diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 0.0);
}

#[test]
fn nine_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = eight_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let initial_angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("initial tree pose");
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        10.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("bounded positive-thickness diagnosis");
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 10.0);
}

#[test]
fn positive_endpoint_memo_cap_rejects_ten_face_tree() {
    let model = deep_strip_model(9);
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        1.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn nine_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = eight_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 10.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let aba_bound = model.bind_pose(&aba_pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
        .unwrap()
        .expect("nine-face boundary");
    assert!(revalidate_tree_hinge_thickness_boundaries_v1(&capability, aba_bound, 0.1).is_none());
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1)
            .is_none()
    );
}

#[test]
fn ten_triangle_positive_thickness_tree_rejects_over_angle() {
    let model = nine_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        8.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn ten_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = nine_hinge_triangle_model();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        8.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 8.0);
}

#[test]
fn positive_endpoint_memo_cap_rejects_eleven_face_tree() {
    let model = deep_strip_model(10);
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        1.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn ten_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = nine_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 8.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
        .unwrap()
        .expect("ten-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba_pose).unwrap(),
            0.1
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1)
            .is_none()
    );
}

#[test]
fn eleven_triangle_positive_thickness_tree_rejects_over_angle() {
    let model = ten_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        6.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn eleven_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = ten_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        6.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 6.0);
}

#[test]
fn positive_endpoint_memo_cap_rejects_twelve_face_tree() {
    let model = deep_strip_model(11);
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        1.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn eleven_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = ten_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 6.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
        .unwrap()
        .expect("eleven-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba_pose).unwrap(),
            0.1,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1,)
            .is_none()
    );
}

#[test]
fn twelve_triangle_positive_thickness_tree_rejects_over_angle() {
    let model = eleven_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        5.000_000_1,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn twelve_triangle_positive_thickness_tree_gets_bounded_certificate() {
    let model = eleven_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        5.0,
        0.01,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(diagnostic.continuous_clearance_certified());
    assert_eq!(diagnostic.safe_stop_angle_degrees(), 5.0);
}

#[test]
fn positive_endpoint_memo_cap_rejects_thirteen_face_tree() {
    let model = deep_strip_model(12);
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        1.0,
        0.1,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!diagnostic.continuous_clearance_certified());
}

#[test]
fn twelve_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = eleven_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 5.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
        .unwrap()
        .expect("twelve-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba_pose).unwrap(),
            0.1,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1,)
            .is_none()
    );
}

#[test]
fn thirteen_triangle_positive_thickness_bounds_and_binding() {
    let model = twelve_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let accepted = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        4.0,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(accepted.continuous_clearance_certified());
    let over = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        4.000_000_1,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!over.continuous_clearance_certified());

    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 4.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
        .unwrap()
        .expect("thirteen-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba).unwrap(),
            0.001,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
            .is_none()
    );
}

#[test]
fn fourteen_triangle_positive_thickness_bounds() {
    let model = thirteen_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let accepted = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        3.0,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(accepted.continuous_clearance_certified());
    let over = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        3.000_000_1,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!over.continuous_clearance_certified());
}

#[test]
fn fourteen_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = thirteen_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 3.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
        .unwrap()
        .expect("fourteen-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba).unwrap(),
            0.001,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
            .is_none()
    );
}

#[test]
fn fifteen_triangle_positive_thickness_bounds_and_work_meter() {
    let model = fourteen_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let accepted = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        2.0,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(accepted.continuous_clearance_certified());
    assert_eq!(accepted.positive_endpoint_memo_pair_entries(), 91);
    assert_eq!(accepted.positive_endpoint_exact_pair_calls(), 0);
    assert!(
        accepted.positive_endpoint_memo_pair_entries()
            + accepted.positive_endpoint_exact_pair_calls()
            <= MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
    );
    let over = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        2.000_000_1,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!over.continuous_clearance_certified());
    assert_eq!(over.positive_endpoint_memo_pair_entries(), 0);
    assert_eq!(over.positive_endpoint_exact_pair_calls(), 0);
}

#[test]
fn fifteen_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = fourteen_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 2.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
        .unwrap()
        .expect("fifteen-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba).unwrap(),
            0.001,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
            .is_none()
    );
}

#[test]
fn sixteen_triangle_positive_thickness_bounds_and_work_meter() {
    let model = fifteen_hinge_triangle_model();
    let (moving, initial) = zero_tree_pose(&model);
    let accepted = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        1.5,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(accepted.continuous_clearance_certified());
    assert_eq!(accepted.positive_endpoint_memo_pair_entries(), 105);
    assert_eq!(accepted.positive_endpoint_exact_pair_calls(), 0);
    assert!(
        accepted.positive_endpoint_memo_pair_entries()
            + accepted.positive_endpoint_exact_pair_calls()
            <= MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
    );
    let over = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        1.500_000_1,
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!over.continuous_clearance_certified());
    assert_eq!(over.positive_endpoint_memo_pair_entries(), 0);
    assert_eq!(over.positive_endpoint_exact_pair_calls(), 0);
}

#[test]
fn sixteen_triangle_boundary_rejects_aba_and_thickness_drift() {
    let model = fifteen_hinge_triangle_model();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 1.5).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let bound = model.bind_pose(&pose).unwrap();
    let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
        .unwrap()
        .expect("sixteen-face boundary");
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(
            &capability,
            model.bind_pose(&aba).unwrap(),
            0.001,
        )
        .is_none()
    );
    assert!(
        revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
            .is_none()
    );
}

#[test]
fn sparse_seventeen_face_tree_is_not_rejected_by_total_pair_count() {
    assert_eq!(MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1, 120);
    let model = sparse_triangle_strip_model(17);
    let (moving, initial) = zero_tree_pose(&model);
    let diagnostic = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        positive_tree_max_angle_degrees_v1(16).unwrap(),
        0.001,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(diagnostic.continuous_clearance_certified());
    assert!(diagnostic.positive_endpoint_memo_pair_entries() <= 120);
    assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
}

#[test]
fn positive_tree_resource_policy_is_branch_independent_and_pair_bounded() {
    for face_count in 10..=16 {
        assert!(positive_tree_resource_premises_v1(
            face_count,
            face_count - 1,
            face_count - 1,
        ));
        let model = branched_triangle_model(face_count, face_count % 2 == 0);
        assert_eq!(model.face_ids().len(), face_count);
        assert_eq!(model.hinges().len(), face_count - 1);
        let maximum_degree = model
            .face_ids()
            .iter()
            .map(|face| {
                model
                    .hinges()
                    .iter()
                    .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                    .count()
            })
            .max()
            .unwrap();
        assert!(maximum_degree >= 3);
    }
    assert_eq!(MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1, 120);
    assert!(positive_tree_resource_premises_v1(17, 16, 16));
    assert!(positive_tree_resource_premises_v1(64, 63, 63));
    assert!(!positive_tree_resource_premises_v1(65, 64, 64));
    assert!(!positive_tree_resource_premises_v1(
        usize::MAX,
        usize::MAX - 1,
        usize::MAX - 1
    ));
}

#[test]
fn positive_tree_angle_policy_is_monotone_with_resource_growth() {
    let maxima = (2..=15)
        .map(|hinges| positive_tree_max_angle_degrees_v1(hinges).unwrap())
        .collect::<Vec<_>>();
    assert!(maxima.windows(2).all(|pair| pair[1] <= pair[0]));
    assert!(positive_tree_max_angle_degrees_v1(1).is_none());
    assert!(positive_tree_max_angle_degrees_v1(16).is_some());
    assert!(positive_tree_max_angle_degrees_v1(63).is_some());
    assert!(positive_tree_max_angle_degrees_v1(64).is_none());
}

#[test]
fn sparse_positive_trees_scale_to_sixty_four_faces_with_zero_candidates() {
    for face_count in [17, 32, 64] {
        let model = sparse_triangle_strip_model(face_count);
        let (moving, initial) = zero_tree_pose(&model);
        let requested = positive_tree_max_angle_degrees_v1(face_count - 1).unwrap();
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(
            diagnostic.continuous_clearance_certified(),
            "face_count={face_count}, diagnostic={diagnostic:?}"
        );
        assert!(diagnostic.positive_endpoint_memo_pair_entries() <= 120);
        assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
    }
}

#[test]
fn dense_sweep_candidate_cap_is_fail_closed_and_order_independent() {
    let mut canonical_candidates = None;
    for reverse_edges in [false, true] {
        let model = branched_triangle_model(26, reverse_edges);
        let (moving, initial) = zero_tree_pose(&model);
        let moving = moving.into_iter().collect::<HashSet<_>>();
        let dense = solve_collective_pose(&model, &initial, &moving, 180.0).unwrap();
        let bound = model.bind_pose(&dense).unwrap();
        let session =
            prepare_positive_thickness_exact_endpoint_session_v2(bound, 1_000_000.0).unwrap();
        let uncapped = session
            .exact_endpoint_candidates_v2(usize::MAX)
            .expect("dense exact candidates within arithmetic work limits");
        assert!(uncapped.len() > MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1);
        if let Some(canonical) = canonical_candidates.as_ref() {
            assert_eq!(&uncapped, canonical);
        } else {
            canonical_candidates = Some(uncapped);
        }
        assert_eq!(
            session.exact_endpoint_candidates_v2(MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1),
            Err(crate::cayley::PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        );
        assert!(positive_endpoint_candidates_v1(&model, &dense, 1_000_000.0).is_none());
    }
}

#[test]
fn endpoint_memo_is_stable_across_hinge_input_and_face_order_permutation() {
    let canonical = fifteen_hinge_triangle_model_with_edge_order(false);
    let reversed = fifteen_hinge_triangle_model_with_edge_order(true);
    for model in [&canonical, &reversed] {
        let (moving, initial) = zero_tree_pose(model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            model,
            &initial,
            &moving,
            1.5,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.positive_endpoint_memo_pair_entries(), 105);
        assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
    }
}

#[test]
fn degenerate_tree_geometry_never_reaches_positive_resource_authority() {
    let vertices = (0..4)
        .map(|index| Vertex {
            id: fixed_id("8e10", index + 1),
            position: Point2::new(index as f64, 0.0),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let pattern = CreasePattern {
        vertices,
        edges: vec![
            Edge {
                id: fixed_id("9e10", 1),
                start: boundary[0],
                end: boundary[1],
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: fixed_id("9e10", 2),
                start: boundary[1],
                end: boundary[2],
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: fixed_id("9e10", 3),
                start: boundary[2],
                end: boundary[3],
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: fixed_id("9e10", 4),
                start: boundary[3],
                end: boundary[0],
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: fixed_id("9e10", 5),
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Mountain,
            },
        ],
    };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("be10", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.snapshot.is_none());

    let vertices = [(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: fixed_id("8e20", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..4)
        .map(|index| Edge {
            id: fixed_id("9e20", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("be20", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.snapshot.is_none());
}

#[test]
fn rectangular_dense_samples_do_not_mint_continuous_authority() {
    for columns in 3..=7 {
        for rows in 3..=7 {
            let (pattern, paper, moving) =
                super::dense_grid_cycle_test_support::rectangular_dense_cycle_pattern(
                    columns, rows,
                );
            let topology = analyze_faces(FaceExtractionInput {
                identity_namespace: ProjectId::new(),
                source_revision: 1,
                paper: &paper,
                pattern: &pattern,
            })
            .snapshot
            .unwrap();
            let geometry = MaterialHingeGraphGeometry::prepare(
                &pattern,
                &paper,
                &topology,
                TreeKinematicsLimits::default(),
            )
            .unwrap();
            let audit =
                MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                    .unwrap();
            let fixed = geometry.face_ids()[0];
            let moving = moving.into_iter().collect::<HashSet<_>>();
            let mut entries = geometry
                .hinges()
                .iter()
                .map(|hinge| HalfAngleRationalEntryInputV1 {
                    edge: hinge.edge(),
                    u_domain: [
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if moving.contains(&hinge.edge()) {
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
                        vec![RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                    denominator_power_coefficients: vec![RationalCoefficientV1 {
                        numerator: if moving.contains(&hinge.edge()) {
                            100
                        } else {
                            1
                        },
                        denominator: 1,
                    }],
                })
                .collect::<Vec<_>>();
            assert_eq!(
                CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    &geometry,
                    &audit,
                    fixed,
                    entries.clone(),
                    CycleScheduleLimitsV1 {
                        max_hinges: geometry.hinges().len() - 1,
                        ..CycleScheduleLimitsV1::default()
                    },
                ),
                Err(ori_kinematics::CycleSchedulePrepareErrorV1::InvalidInput)
            );
            let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
                &geometry,
                &audit,
                fixed,
                entries.clone(),
                CycleScheduleLimitsV1::default(),
            )
            .unwrap();
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    fixed,
                    &schedule,
                    1.0e-8,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 0,
                        max_leaves: 1,
                        max_work: 1,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .unwrap();
            let face_count = columns * rows;
            let expected_pairs = face_count * (face_count - 1) / 2;
            let initial_angles = schedule.evaluate(0.0).unwrap();
            let initial_pose = geometry
                .solve_closed(&audit, fixed, &initial_angles, 1.0e-8)
                .unwrap();
            assert!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &initial_pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1 {
                        max_unordered_face_pairs: expected_pairs - 1,
                        ..PositiveThicknessGraphLimitsV1::default()
                    },
                )
                .is_err()
            );
            assert_eq!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &initial_pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1 {
                        max_unordered_face_pairs: expected_pairs,
                        ..PositiveThicknessGraphLimitsV1::default()
                    },
                )
                .unwrap()
                .analyzed_unordered_face_pairs(),
                expected_pairs
            );
            for thickness in [0.1, 1.0, 3.0] {
                for progress in [0.0, 0.5, 1.0] {
                    let angles = schedule.evaluate(progress).unwrap();
                    let pose = geometry
                        .solve_closed(&audit, fixed, &angles, 1.0e-8)
                        .unwrap();
                    prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    thickness,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("{columns}x{rows}, thickness {thickness}, progress {progress}: {error:?}")
                });
                }
                let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                    &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
                );
                assert!(
                    diagnostic.continuous_certificate_model_id().is_none(),
                    "{columns}x{rows}, thickness {thickness}: three exact samples are not a continuous proof"
                );
                assert_eq!(diagnostic.pair_work(), 0);
            }
            let rejected = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
            );
            assert!(rejected.continuous_certificate_model_id().is_none());
            assert_eq!(rejected.pair_work(), 0);
            if (columns, rows) == (3, 4) {
                let foreign_geometry = MaterialHingeGraphGeometry::prepare(
                    &pattern,
                    &paper,
                    &topology,
                    TreeKinematicsLimits::default(),
                )
                .unwrap();
                assert!(
                    diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                        &foreign_geometry,
                        &audit,
                        fixed,
                        &schedule,
                        &closure,
                        0.1,
                        1,
                    )
                    .continuous_certificate_model_id()
                    .is_none()
                );
            }
            if columns == rows && columns >= 6 {
                for entry in &mut entries {
                    if moving.contains(&entry.edge) {
                        entry.denominator_power_coefficients[0].numerator = 1;
                    }
                }
                let collision_schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    &geometry,
                    &audit,
                    fixed,
                    entries,
                    CycleScheduleLimitsV1::default(),
                )
                .unwrap();
                let collision_closure = geometry
                    .prove_dyadic_schedule_closure_v1(
                        &audit,
                        fixed,
                        &collision_schedule,
                        1.0e-8,
                        DyadicIntervalClosureLimitsV1 {
                            max_depth: 0,
                            max_leaves: 1,
                            max_work: 1,
                            schedule_limits: CycleScheduleLimitsV1::default(),
                        },
                    )
                    .unwrap();
                assert!(
                    diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                        &geometry,
                        &audit,
                        fixed,
                        &collision_schedule,
                        &collision_closure,
                        0.1,
                        1,
                    )
                    .continuous_certificate_model_id()
                    .is_none(),
                    "{columns}x{rows} swept collision must fail closed"
                );
            }
        }
    }
}

#[test]
fn orthogonal_axis_rank_four_dense_samples_fail_closed() {
    let (pattern, paper, horizontal, vertical) =
        super::dense_grid_cycle_test_support::orthogonal_dense_cycle_pattern(3, 3);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: ProjectId::new(),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (9, 12)
    );
    assert_eq!(audit.closure_hinges().len(), 4);
    let horizontal_axis = geometry
        .hinges()
        .iter()
        .find(|hinge| horizontal.contains(&hinge.edge()))
        .unwrap()
        .axis();
    let vertical_axis = geometry
        .hinges()
        .iter()
        .find(|hinge| vertical.contains(&hinge.edge()))
        .unwrap()
        .axis();
    assert!(
        (horizontal_axis.x() * vertical_axis.x()
            + horizontal_axis.y() * vertical_axis.y()
            + horizontal_axis.z() * vertical_axis.z())
        .abs()
            <= f64::EPSILON,
        "the dense carrier contains exact orthogonal hinge axes"
    );
    let fixed = geometry.face_ids()[0];
    for moving in [horizontal, vertical] {
        let moving = moving.into_iter().collect::<HashSet<_>>();
        let entries = geometry
            .hinges()
            .iter()
            .map(|hinge| HalfAngleRationalEntryInputV1 {
                edge: hinge.edge(),
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if moving.contains(&hinge.edge()) {
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
                    vec![RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: if moving.contains(&hinge.edge()) {
                        100
                    } else {
                        1
                    },
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        let mut reversed = entries.clone();
        reversed.reverse();
        assert_eq!(
            CanonicalCycleScheduleV1::prepare_half_angle_rational(
                &geometry,
                &audit,
                fixed,
                reversed,
                CycleScheduleLimitsV1::default(),
            ),
            Err(ori_kinematics::CycleSchedulePrepareErrorV1::NonCanonical)
        );
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            entries.clone(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let initial = schedule.evaluate(0.0).unwrap();
        let pose = geometry
            .solve_closed(&audit, fixed, &initial, 1.0e-8)
            .unwrap();
        assert_eq!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 35,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            )
            .unwrap_err(),
            crate::PositiveThicknessGraphProofErrorV1::ResourceLimit
        );
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        for thickness in [0.1, 1.0, 3.0] {
            let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
            );
            assert!(diagnostic.continuous_certificate_model_id().is_none());
            assert_eq!(diagnostic.pair_work(), 0);
        }
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
        let mut collision_entries = entries;
        for entry in &mut collision_entries {
            if moving.contains(&entry.edge) {
                entry.denominator_power_coefficients[0].numerator = 1;
            }
        }
        let collision_schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            collision_entries,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let collision_closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &collision_schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry,
                &audit,
                fixed,
                &collision_schedule,
                &collision_closure,
                0.1,
                1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
        let foreign = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &foreign, &audit, fixed, &schedule, &closure, 0.1, 1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
}

#[test]
fn sixty_degree_axis_rank_four_dense_graph_remains_exact_and_fail_closed() {
    let (pattern, paper, horizontal, vertical) =
        super::dense_grid_cycle_test_support::oblique_dense_cycle_pattern(3, 3);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: ProjectId::new(),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (9, 12)
    );
    assert_eq!(audit.closure_hinges().len(), 4);
    let first_axis = |edges: &[ori_domain::EdgeId]| {
        geometry
            .hinges()
            .iter()
            .find(|hinge| edges.contains(&hinge.edge()))
            .unwrap()
            .axis()
    };
    let a = first_axis(&horizontal);
    let b = first_axis(&vertical);
    let dot = a.x() * b.x() + a.y() * b.y() + a.z() * b.z();
    assert!(
        (dot.abs() - 0.5).abs() <= 1.0e-12,
        "axes meet at 60 degrees"
    );
    let fixed = geometry.face_ids()[0];
    for (family_index, moving) in [vertical].into_iter().enumerate() {
        let moving = moving.into_iter().collect::<HashSet<_>>();
        let make_entries = |denominator: i64| {
            geometry
                .hinges()
                .iter()
                .map(|hinge| HalfAngleRationalEntryInputV1 {
                    edge: hinge.edge(),
                    u_domain: [
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if moving.contains(&hinge.edge()) {
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
                        vec![RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                    denominator_power_coefficients: vec![RationalCoefficientV1 {
                        numerator: if moving.contains(&hinge.edge()) {
                            denominator
                        } else {
                            1
                        },
                        denominator: 1,
                    }],
                })
                .collect::<Vec<_>>()
        };
        let mut entries = make_entries(4);
        for entry in &mut entries {
            if moving.contains(&entry.edge) {
                entry.numerator_power_coefficients[1].numerator = 0;
            }
        }
        if family_index == 0 {
            let mut reversed = entries.clone();
            reversed.reverse();
            assert_eq!(
                CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    &geometry,
                    &audit,
                    fixed,
                    reversed,
                    CycleScheduleLimitsV1::default(),
                ),
                Err(ori_kinematics::CycleSchedulePrepareErrorV1::NonCanonical)
            );
        }
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            entries,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        let pose = geometry
            .solve_closed(&audit, fixed, &schedule.evaluate(0.0).unwrap(), 1.0e-8)
            .unwrap();
        assert_eq!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 35,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            )
            .unwrap_err(),
            crate::PositiveThicknessGraphProofErrorV1::ResourceLimit
        );
        for thickness in [0.1, 1.0, 3.0] {
            for progress in [0.0, 0.5, 1.0] {
                let angles = schedule.evaluate(progress).unwrap();
                let sample_pose = geometry
                    .solve_closed(&audit, fixed, &angles, 1.0e-8)
                    .unwrap();
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &sample_pose,
                    thickness,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("family {family_index}, thickness {thickness}, progress {progress}: {error:?}")
                });
            }
            let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
            );
            assert!(diagnostic.continuous_certificate_model_id().is_some());
            assert_eq!(diagnostic.pair_work(), 36);
        }
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
        let foreign = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &foreign, &audit, fixed, &schedule, &closure, 0.1, 1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
        let collision_schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            make_entries(1),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let collision_closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &collision_schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry,
                &audit,
                fixed,
                &collision_schedule,
                &collision_closure,
                0.1,
                1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
}

#[test]
fn parametric_oblique_rank_four_carriers_preserve_static_positive_authority() {
    for angle_degrees in [30.0_f64, 45.0, 120.0] {
        let (pattern, paper, horizontal, vertical) =
            super::dense_grid_cycle_test_support::angled_dense_cycle_pattern(3, 3, angle_degrees);
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: ProjectId::new(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .unwrap();
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 4);
        let axis = |edges: &[ori_domain::EdgeId]| {
            geometry
                .hinges()
                .iter()
                .find(|hinge| edges.contains(&hinge.edge()))
                .unwrap()
                .axis()
        };
        let a = axis(&horizontal);
        let b = axis(&vertical);
        let dot = (a.x() * b.x() + a.y() * b.y() + a.z() * b.z()).abs();
        assert!((dot - angle_degrees.to_radians().cos().abs()).abs() <= 1.0e-12);
        let fixed = geometry.face_ids()[0];
        let entries = geometry
            .hinges()
            .iter()
            .map(|hinge| CycleScheduleEntryInputV1 {
                edge: hinge.edge(),
                initial_angle_degrees_bits: 0.0_f64.to_bits(),
                chebyshev_coefficients: vec![RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        for thickness in [0.1, 1.0, 3.0] {
            let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
            );
            assert!(diagnostic.continuous_certificate_model_id().is_some());
            assert_eq!(diagnostic.pair_work(), 36);
        }
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
    }
}

#[test]
fn non_grid_rank_four_cycle_basis_is_simultaneous_bounded_and_issuer_bound() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    assert_eq!(audit.closure_hinges().len(), 4);
    assert_ne!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (9, 12)
    );
    let basis = geometry
        .extract_canonical_cycle_basis_v1(&audit, ori_kinematics::CycleBasisLimitsV1::default())
        .unwrap();
    assert_eq!(basis.cycles().len(), 4);
    assert!(basis.is_for_geometry(&geometry));
    for (cycle, closure_edge) in basis.cycles().iter().zip(audit.closure_hinges()) {
        assert_eq!(cycle.last(), Some(closure_edge));
    }
    let total_edges = basis.cycles().iter().map(Vec::len).sum::<usize>();
    assert!(matches!(
        geometry.extract_canonical_cycle_basis_v1(
            &audit,
            ori_kinematics::CycleBasisLimitsV1 {
                max_cycles: 3,
                ..ori_kinematics::CycleBasisLimitsV1::default()
            },
        ),
        Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
    ));
    assert!(matches!(
        geometry.extract_canonical_cycle_basis_v1(
            &audit,
            ori_kinematics::CycleBasisLimitsV1 {
                max_total_cycle_edges: total_edges - 1,
                ..ori_kinematics::CycleBasisLimitsV1::default()
            },
        ),
        Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
    ));
    let simultaneous = geometry
        .prove_simultaneous_cycle_basis_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-9,
            ori_kinematics::CycleBasisLimitsV1 {
                max_cycles: 4,
                max_edges_per_cycle: basis.cycles().iter().map(Vec::len).max().unwrap(),
                max_total_cycle_edges: total_edges,
            },
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    assert_eq!(simultaneous.basis().cycles(), basis.cycles());
    assert!(simultaneous.closure().every_leaf_covers_graph_v1(&geometry));
    for thickness in [0.1, 1.0, 3.0] {
        let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            simultaneous.closure(),
            thickness,
            32,
        );
        assert!(
            diagnostic.continuous_certificate_model_id().is_none(),
            "cycle-basis closure is not an all-pair continuous-clearance theorem"
        );
    }
    assert!(
        diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            simultaneous.closure(),
            10_000.0,
            32,
        )
        .continuous_certificate_model_id()
        .is_none()
    );
    let (foreign, _, _, _) = rational_cycle_bay_geometry(4, false);
    assert!(!basis.is_for_geometry(&foreign));
}

#[test]
fn non_grid_rank_eight_to_thirty_two_basis_scale_to_exact_all_pair_limits() {
    for rank in [8usize, 16, 32] {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(rank, false);
        assert_eq!(audit.closure_hinges().len(), rank);
        assert!(supports_scheduled_positive_thickness_path_v1(
            &geometry, &audit, fixed, &schedule,
        ));
        let wrong_fixed = *geometry
            .face_ids()
            .iter()
            .find(|face| **face != fixed)
            .unwrap();
        assert!(!supports_scheduled_positive_thickness_path_v1(
            &geometry,
            &audit,
            wrong_fixed,
            &schedule,
        ));
        let basis_limits = ori_kinematics::CycleBasisLimitsV1::default();
        let basis = geometry
            .extract_canonical_cycle_basis_v1(&audit, basis_limits)
            .unwrap();
        let repeated = geometry
            .extract_canonical_cycle_basis_v1(&audit, basis_limits)
            .unwrap();
        assert_eq!(basis.cycles(), repeated.cycles());
        let total_edges = basis.cycles().iter().map(Vec::len).sum::<usize>();
        let max_cycle_edges = basis.cycles().iter().map(Vec::len).max().unwrap();
        for limits in [
            ori_kinematics::CycleBasisLimitsV1 {
                max_cycles: rank - 1,
                ..basis_limits
            },
            ori_kinematics::CycleBasisLimitsV1 {
                max_edges_per_cycle: max_cycle_edges - 1,
                ..basis_limits
            },
            ori_kinematics::CycleBasisLimitsV1 {
                max_total_cycle_edges: total_edges - 1,
                ..basis_limits
            },
        ] {
            assert!(matches!(
                geometry.extract_canonical_cycle_basis_v1(&audit, limits),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
            ));
        }
        let closure_limits = DyadicIntervalClosureLimitsV1 {
            max_depth: rank.ilog2(),
            max_leaves: rank,
            max_work: rank,
            schedule_limits: CycleScheduleLimitsV1::default(),
        };
        let simultaneous = geometry
            .prove_simultaneous_cycle_basis_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-9,
                ori_kinematics::CycleBasisLimitsV1 {
                    max_cycles: rank,
                    max_edges_per_cycle: max_cycle_edges,
                    max_total_cycle_edges: total_edges,
                },
                closure_limits,
            )
            .unwrap();
        assert_eq!(simultaneous.closure().leaves().len(), rank);
        assert!(matches!(
            geometry.prove_simultaneous_cycle_basis_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-9,
                basis_limits,
                DyadicIntervalClosureLimitsV1 {
                    max_work: closure_limits.max_work - 1,
                    ..closure_limits
                },
            ),
            Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
        ));
        let initial = schedule.evaluate(0.0).unwrap();
        let pose = geometry
            .solve_closed(&audit, fixed, &initial, 1.0e-9)
            .unwrap();
        let face_count = geometry.face_ids().len();
        let expected_pairs = face_count * (face_count - 1) / 2;
        assert!(matches!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: expected_pairs - 1,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            ),
            Err(crate::PositiveThicknessGraphProofErrorV1::ResourceLimit)
        ));
        assert_eq!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: expected_pairs,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            )
            .unwrap()
            .analyzed_unordered_face_pairs(),
            expected_pairs
        );
        for thickness in [0.1, 1.0, 3.0] {
            assert!(
                diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &schedule,
                    simultaneous.closure(),
                    thickness,
                    32,
                )
                .continuous_certificate_model_id()
                .is_none(),
                "bounded cycle-basis closure and static solid clearance cannot mint continuous authority"
            );
        }
        assert!(
            diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                simultaneous.closure(),
                10_000.0,
                32,
            )
            .continuous_certificate_model_id()
            .is_none()
        );
        let (foreign, _, _, _) = rational_cycle_bay_geometry(rank, false);
        assert!(!basis.is_for_geometry(&foreign));
    }
}

#[test]
fn continuous_layer_transport_binds_every_dyadic_transition_and_fails_closed() {
    use ori_foldability::{
        FacePairOrderSnapshot, FacewiseProofSummary, GlobalFlatFoldabilityModelId,
        GlobalFlatFoldabilityProvenance, LAYER_ORDER_MODEL_ID, LayerFace, LayerOrderDerivation,
        LayerOrderProvenance, LayerOrderSnapshot,
    };
    use ori_topology::FaceKey;

    // A rank-64 carrier has 65 faces and at most 2,080 unordered face
    // pairs. Cancellation is observed before retaining any of its 65
    // transition witnesses or performing one hash operation.
    assert!(matches!(
        crate::preflight_continuous_layer_transport_work_v1(
            65,
            2_080,
            crate::ContinuousLayerTransportLimitsV1 {
                max_transitions: 0,
                max_pair_orders: usize::MAX,
            },
        ),
        Err(crate::ContinuousLayerTransportErrorV1::ResourceLimit)
    ));

    for rank in [4, 8, 16, 32] {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(rank, false);
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: rank.ilog2(),
                    max_leaves: rank,
                    max_work: rank,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        let faces = geometry
            .face_ids()
            .iter()
            .enumerate()
            .map(|(index, face_id)| LayerFace {
                face_id: *face_id,
                face_key: FaceKey([index as u8; 32]),
            })
            .collect::<Vec<_>>();
        let mut source = LayerOrderSnapshot {
            model_id: LAYER_ORDER_MODEL_ID,
            material_faces: faces.clone(),
            global_bottom_to_top: None,
            provenance: LayerOrderProvenance {
                source: GlobalFlatFoldabilityProvenance {
                    identity_namespace: Some(fixed_id("b601", 1)),
                    source_revision: 1,
                    source_fingerprint: Some(ori_foldability::FoldModelFingerprintV1([7; 32])),
                    model_id: GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1,
                },
                derivation: LayerOrderDerivation::FacewiseCertificate {
                    reference_face: faces[0],
                    overlap_cell_count: 0,
                    constraint_count: 2,
                },
            },
            reference_face: Some(faces[0]),
            folded_faces: Vec::new(),
            overlap_cells: Vec::new(),
            face_pair_orders: vec![
                FacePairOrderSnapshot {
                    lower_face: faces[0],
                    upper_face: faces[1],
                    supporting_cells: Vec::new(),
                },
                FacePairOrderSnapshot {
                    lower_face: faces[2],
                    upper_face: faces[3],
                    supporting_cells: Vec::new(),
                },
            ],
            proof_summary: Some(FacewiseProofSummary {
                material_faces: faces.len(),
                overlap_face_pairs: 2,
                overlap_cells: 0,
                constraints: 2,
                search_nodes: 1,
                maximum_ply: 2,
                certificate_bytes: 1,
            }),
        };
        let mapping = faces
            .iter()
            .enumerate()
            .map(|(index, face)| (face.face_id, faces[(index + 1) % faces.len()].face_id))
            .collect::<Vec<_>>();
        let first = (mapping[0].1, mapping[1].1);
        let second = (mapping[2].1, mapping[3].1);
        let transitions = (0..=closure.leaves().len())
            .map(|index| {
                if index % 2 == 0 {
                    vec![first, second]
                } else {
                    vec![second, first]
                }
            })
            .collect::<Vec<_>>();
        let exact = crate::ContinuousLayerTransportLimitsV1 {
            max_transitions: transitions.len(),
            max_pair_orders: transitions.len() * 2,
        };
        let proof = crate::prove_continuous_layer_transport_v1(
            &geometry,
            &source,
            &mapping,
            &schedule,
            &closure,
            &transitions,
            exact,
        )
        .unwrap();
        if rank <= 32 {
            let axes = [
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ];
            let mut selected = None;
            for order in &source.face_pair_orders {
                let mut candidate = source.clone();
                candidate.face_pair_orders = vec![order.clone()];
                for axis in axes {
                    if let Ok(derived) = crate::derive_continuous_layer_transport_from_poses_v1(
                        crate::ContinuousLayerTransportFromPosesInputV1 {
                            geometry: &geometry,
                            audit: &audit,
                            source: &candidate,
                            source_to_target: &mapping,
                            schedule: &schedule,
                            closure: &closure,
                            separation_axis: axis,
                            tolerance: 1.0e-9,
                            limits: exact,
                        },
                    ) {
                        selected = Some((derived, candidate, axis));
                        break;
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
            let (derived, derivation_source, separation_axis) =
                selected.expect("one exact pose-derived partial order");
            assert_eq!(
                derived.transition_hashes().len(),
                closure.leaves().len() + 1
            );
            assert!(
                derived
                    .transition_hashes()
                    .windows(2)
                    .any(|pair| pair[0] != pair[1])
            );
            let repeated = crate::derive_continuous_layer_transport_from_poses_v1(
                crate::ContinuousLayerTransportFromPosesInputV1 {
                    geometry: &geometry,
                    audit: &audit,
                    source: &derivation_source,
                    source_to_target: &mapping,
                    schedule: &schedule,
                    closure: &closure,
                    separation_axis,
                    tolerance: 1.0e-9,
                    limits: exact,
                },
            )
            .unwrap();
            assert_eq!(derived.transition_hashes(), repeated.transition_hashes());
            if rank == 32 {
                let (foreign, foreign_audit, _, _) = rational_cycle_bay_geometry(rank, false);
                assert!(matches!(
                    crate::derive_continuous_layer_transport_from_poses_v1(
                        crate::ContinuousLayerTransportFromPosesInputV1 {
                            geometry: &foreign,
                            audit: &foreign_audit,
                            source: &derivation_source,
                            source_to_target: &mapping,
                            schedule: &schedule,
                            closure: &closure,
                            separation_axis,
                            tolerance: 1.0e-9,
                            limits: exact,
                        },
                    ),
                    Err(crate::ContinuousLayerTransportErrorV1::BindingMismatch)
                ));
            }
            assert!(matches!(
                crate::derive_continuous_layer_transport_from_poses_v1(
                    crate::ContinuousLayerTransportFromPosesInputV1 {
                        geometry: &geometry,
                        audit: &audit,
                        source: &derivation_source,
                        source_to_target: &mapping,
                        schedule: &schedule,
                        closure: &closure,
                        separation_axis,
                        tolerance: 1.0e9,
                        limits: exact,
                    },
                ),
                Err(crate::ContinuousLayerTransportErrorV1::AmbiguousOrder)
            ));
            assert!(matches!(
                crate::derive_continuous_layer_transport_from_poses_v1(
                    crate::ContinuousLayerTransportFromPosesInputV1 {
                        geometry: &geometry,
                        audit: &audit,
                        source: &derivation_source,
                        source_to_target: &mapping,
                        schedule: &schedule,
                        closure: &closure,
                        separation_axis: [0.0; 3],
                        tolerance: 1.0e-9,
                        limits: exact,
                    },
                ),
                Err(crate::ContinuousLayerTransportErrorV1::BindingMismatch)
            ));
            assert!(matches!(
                crate::derive_continuous_layer_transport_from_poses_v1(
                    crate::ContinuousLayerTransportFromPosesInputV1 {
                        geometry: &geometry,
                        audit: &audit,
                        source: &derivation_source,
                        source_to_target: &mapping,
                        schedule: &schedule,
                        closure: &closure,
                        separation_axis,
                        tolerance: 1.0e-9,
                        limits: crate::ContinuousLayerTransportLimitsV1 {
                            max_pair_orders: (closure.leaves().len() + 1)
                                * derivation_source.face_pair_orders.len()
                                - 1,
                            ..exact
                        },
                    },
                ),
                Err(crate::ContinuousLayerTransportErrorV1::ResourceLimit)
            ));
            let selected_order = derivation_source.face_pair_orders[0].clone();
            let mut cyclic = derivation_source.clone();
            cyclic.face_pair_orders.push(FacePairOrderSnapshot {
                lower_face: selected_order.upper_face,
                upper_face: selected_order.lower_face,
                supporting_cells: Vec::new(),
            });
            assert!(matches!(
                crate::derive_continuous_layer_transport_from_poses_v1(
                    crate::ContinuousLayerTransportFromPosesInputV1 {
                        geometry: &geometry,
                        audit: &audit,
                        source: &cyclic,
                        source_to_target: &mapping,
                        schedule: &schedule,
                        closure: &closure,
                        separation_axis,
                        tolerance: 1.0e-9,
                        limits: crate::ContinuousLayerTransportLimitsV1 {
                            max_pair_orders: exact.max_pair_orders * 2,
                            ..exact
                        },
                    },
                ),
                Err(crate::ContinuousLayerTransportErrorV1::Crossing)
            ));
        }
        assert_eq!(proof.transition_hashes().len(), transitions.len());
        assert!(
            proof
                .transition_hashes()
                .windows(2)
                .all(|pair| pair[0] == pair[1])
        );
        assert_eq!(geometry.face_ids().len(), 1 + rank * 3);
        assert_eq!(geometry.hinges().len(), rank * 4);
        assert!(proof.is_for(&geometry, &source, &schedule, &closure));
        assert!(!proof.is_for(&geometry, &source.clone(), &schedule, &closure));
        assert!(proof.matches_source_content_v1(&source.clone()));
        source.provenance.source.source_revision += 1;
        assert!(!proof.is_for(&geometry, &source, &schedule, &closure));
        assert!(!proof.matches_source_content_v1(&source));
        source.provenance.source.source_revision -= 1;
        let mut reversed = transitions.clone();
        reversed[2] = vec![(first.1, first.0), second];
        assert!(matches!(
            crate::prove_continuous_layer_transport_v1(
                &geometry, &source, &mapping, &schedule, &closure, &reversed, exact,
            ),
            Err(crate::ContinuousLayerTransportErrorV1::Crossing)
        ));
        let mut ambiguous = transitions.clone();
        ambiguous[1].pop();
        assert!(matches!(
            crate::prove_continuous_layer_transport_v1(
                &geometry, &source, &mapping, &schedule, &closure, &ambiguous, exact,
            ),
            Err(crate::ContinuousLayerTransportErrorV1::AmbiguousOrder)
        ));
        let mut collision = transitions.clone();
        collision[1][0] = (first.0, first.0);
        assert!(matches!(
            crate::prove_continuous_layer_transport_v1(
                &geometry, &source, &mapping, &schedule, &closure, &collision, exact,
            ),
            Err(crate::ContinuousLayerTransportErrorV1::Collision)
        ));
        let (foreign, _, _, _) = rational_cycle_bay_geometry(rank, false);
        assert!(!proof.is_for(&foreign, &source, &schedule, &closure));
        assert!(matches!(
            crate::prove_continuous_layer_transport_v1(
                &geometry,
                &source,
                &mapping,
                &schedule,
                &closure,
                &transitions,
                crate::ContinuousLayerTransportLimitsV1 {
                    max_pair_orders: transitions.len() * 2 - 1,
                    ..exact
                },
            ),
            Err(crate::ContinuousLayerTransportErrorV1::ResourceLimit)
        ));
        assert!(matches!(
            crate::prove_continuous_layer_transport_v1(
                &geometry,
                &source,
                &mapping,
                &schedule,
                &closure,
                &transitions,
                crate::ContinuousLayerTransportLimitsV1 {
                    max_transitions: 0,
                    ..exact
                },
            ),
            Err(crate::ContinuousLayerTransportErrorV1::ResourceLimit)
        ));
    }
}

#[test]
fn miura_rank_four_fixture_keeps_stationary_global_layer_authority() {
    let (pattern, paper, horizontal, _) =
        super::dense_grid_cycle_test_support::three_by_three_miura_authority_pattern();
    let project = fixed_id("b602", 1);
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    let topology = report.snapshot.expect("convex Miura topology");
    let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
    let global = ori_foldability::analyze_global_flat_foldability(
        ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
            project, &paper, &pattern, &topology, &local,
        ),
        ori_foldability::GlobalFlatFoldabilityLimits::default(),
    )
    .unwrap();
    assert!(global.layer_order().is_some(), "{:?}", global.outcome_v2());

    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = topology.faces[0].id;
    let hinge_edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    // The square 3M1V tangent-half-angle equations degenerate to one
    // active collinear carrier and its orthogonal zero pair. Propagating
    // that constraint through the shared grid selects the first complete
    // horizontal row: three segments, all with p/q = 1, and nine zeros.
    let active = horizontal.into_iter().take(3).collect::<HashSet<_>>();
    let selected: [(ori_domain::EdgeId, bool); 3] = active
        .iter()
        .copied()
        .map(|edge| (edge, true))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let petal_candidate = crate::regular_quad_petal_ratio_candidates_v1(selected)[0];
    let petal_schedules = crate::prepare_regular_quad_petal_schedules_v1(
        &geometry,
        &audit,
        fixed,
        &petal_candidate,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        petal_schedules[0].evaluate(1.0),
        petal_schedules[1].evaluate(0.0)
    );
    assert_eq!(
        petal_schedules[1].evaluate(1.0),
        petal_schedules[2].evaluate(0.0)
    );
    assert!(petal_schedules.iter().all(|petal_schedule| {
        geometry
            .solve_closed(
                &audit,
                fixed,
                &petal_schedule.evaluate(1.0).unwrap(),
                1.0e-9,
            )
            .is_ok()
    }));
    let endpoint = CanonicalHingeAngles::new(
        hinge_edges
            .iter()
            .map(|edge| {
                HingeAngle::new(*edge, if active.contains(edge) { 90.0 } else { 0.0 }).unwrap()
            })
            .collect(),
    )
    .unwrap();
    geometry
        .solve_closed(&audit, fixed, &endpoint, 1.0e-9)
        .unwrap();
    // The moving petal remains endpoint/closure evidence only. Reusable
    // positive-thickness and layer authority below uses the exact stationary
    // schedule until an open-interval all-pair theorem exists.
    let mut entries = hinge_edges
        .iter()
        .map(|edge| ori_kinematics::HalfAngleRationalEntryInputV1 {
            edge: *edge,
            u_domain: [
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                numerator: if active.contains(edge) { 100 } else { 1 },
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-9,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    assert!(closure.every_leaf_covers_graph_v1(&geometry));
    let expected_pairs = geometry.face_ids().len() * (geometry.face_ids().len() - 1) / 2;
    for thickness in [0.1, 1.0, 3.0] {
        let diagnostic = diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
        );
        assert!(diagnostic.continuous_certificate_model_id().is_some());
        assert_eq!(diagnostic.pair_work(), expected_pairs);
        assert_eq!(
            diagnostic.positive_thickness_bits(),
            Some(thickness.to_bits())
        );
        let certificate = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
        )
        .unwrap();
        assert_eq!(
            certificate.checked_deep_retained_bytes_v1(),
            Some(std::mem::size_of::<PositiveThicknessContinuousCertificateV1>())
        );
        assert!(certificate.is_for(&geometry, &audit, fixed, &schedule, &closure, thickness));
    }
    assert!(
        diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
        )
        .continuous_certificate_model_id()
        .is_none()
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, 10_000.0, 1,
        )
        .is_none()
    );
    let bound_certificate = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry, &audit, fixed, &schedule, &closure, 0.1, 1,
    )
    .unwrap();
    let source = global.layer_order().unwrap();
    let cell_work = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len() * cell.bottom_to_top_faces.len())
        .sum::<usize>()
        * (closure.leaves().len() + 1);
    let cell_limits = crate::GeneralCellTransportLimitsV1 {
        max_transitions: closure.leaves().len() + 1,
        max_cells: source.overlap_cells.len(),
        max_layer_records: source
            .overlap_cells
            .iter()
            .map(|cell| cell.bottom_to_top_faces.len())
            .sum(),
        max_boundary_samples: cell_work,
    };
    let petal_closure_limits = DyadicIntervalClosureLimitsV1 {
        max_depth: 8,
        max_leaves: 256,
        max_work: 1_000_000,
        schedule_limits: CycleScheduleLimitsV1::default(),
    };
    assert!(
        crate::issue_regular_quad_petal_chained_authority_v1(
            &geometry,
            &audit,
            source,
            fixed,
            selected,
            0.1,
            1.0e-9,
            CycleScheduleLimitsV1::default(),
            petal_closure_limits,
        )
        .is_none(),
        "a three-stage moving petal schedule has no sampled fast-path authority"
    );
    let petal_closures = petal_schedules
        .iter()
        .map(|schedule| {
            geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    fixed,
                    schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 8,
                        max_leaves: 256,
                        max_work: 1_000_000,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        petal_schedules
            .iter()
            .zip(&petal_closures)
            .all(|(schedule, closure)| {
                certify_canonical_positive_thickness_cycle_schedule_path_v1(
                    &geometry, &audit, fixed, schedule, closure, 0.1, 1,
                )
                .is_none()
            }),
        "finite per-stage petal samples cannot mint continuous authority"
    );
    for thickness in [0.1, 1.0, 3.0] {
        let authority = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, thickness, 1,
        )
        .unwrap();
        let cell_proof = crate::certify_general_multi_face_cell_transport_v1(
            crate::GeneralCellTransportInputV1 {
                geometry: &geometry,
                audit: &audit,
                source,
                schedule: &schedule,
                closure: &closure,
                positive_continuous: &authority,
                paper_thickness_mm: thickness,
                tolerance: 1.0e-9,
                limits: cell_limits,
            },
        )
        .unwrap();
        assert!(cell_proof.is_for(&geometry, source, &schedule, &closure, thickness));
    }
    assert!(matches!(
        crate::certify_general_multi_face_cell_transport_v1(crate::GeneralCellTransportInputV1 {
            geometry: &geometry,
            audit: &audit,
            source,
            schedule: &schedule,
            closure: &closure,
            positive_continuous: &bound_certificate,
            paper_thickness_mm: 0.1,
            tolerance: 1.0e-9,
            limits: crate::GeneralCellTransportLimitsV1 {
                max_boundary_samples: cell_work - 1,
                ..cell_limits
            },
        },),
        Err(crate::GeneralCellTransportErrorV1::ResourceLimit)
    ));
    assert_eq!(
        crate::certify_general_multi_face_cell_transport_v1(crate::GeneralCellTransportInputV1 {
            geometry: &geometry,
            audit: &audit,
            source,
            schedule: &schedule,
            closure: &closure,
            positive_continuous: &bound_certificate,
            paper_thickness_mm: 0.1,
            tolerance: 1.0e9,
            limits: cell_limits,
        },)
        .expect_err("non-model tolerance must fail closed"),
        crate::GeneralCellTransportErrorV1::BindingMismatch,
    );
    let bound_cell_proof =
        crate::certify_general_multi_face_cell_transport_v1(crate::GeneralCellTransportInputV1 {
            geometry: &geometry,
            audit: &audit,
            source,
            schedule: &schedule,
            closure: &closure,
            positive_continuous: &bound_certificate,
            paper_thickness_mm: 0.1,
            tolerance: 1.0e-9,
            limits: cell_limits,
        })
        .unwrap();
    let mut continuation_entries = hinge_edges
        .iter()
        .map(|edge| ori_kinematics::HalfAngleRationalEntryInputV1 {
            edge: *edge,
            u_domain: [
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                numerator: if active.contains(edge) { 100 } else { 1 },
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    continuation_entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let continuation = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        continuation_entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(schedule.evaluate(1.0), continuation.evaluate(0.0));
    let continuation_closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &continuation,
            1.0e-9,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    let continuation_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed,
        &continuation,
        &continuation_closure,
        0.1,
        1,
    )
    .unwrap();
    let mut flatten_entries = hinge_edges
        .iter()
        .map(|edge| ori_kinematics::HalfAngleRationalEntryInputV1 {
            edge: *edge,
            u_domain: [
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                ori_kinematics::RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                numerator: if active.contains(edge) { 100 } else { 1 },
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    flatten_entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let flatten = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        flatten_entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(continuation.evaluate(1.0), flatten.evaluate(0.0));
    let flatten_closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &flatten,
            1.0e-9,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .unwrap();
    let flatten_positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed,
        &flatten,
        &flatten_closure,
        0.1,
        1,
    )
    .unwrap();
    let continuation_work = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len() * cell.bottom_to_top_faces.len())
        .sum::<usize>()
        * (continuation_closure.leaves().len() + 1);
    let continuation_limits = crate::GeneralCellTransportLimitsV1 {
        max_transitions: continuation_closure.leaves().len() + 1,
        max_boundary_samples: continuation_work,
        ..cell_limits
    };
    let first_input = || crate::GeneralCellTransportInputV1 {
        geometry: &geometry,
        audit: &audit,
        source,
        schedule: &schedule,
        closure: &closure,
        positive_continuous: &bound_certificate,
        paper_thickness_mm: 0.1,
        tolerance: 1.0e-9,
        limits: cell_limits,
    };
    let continuation_input = |limits| crate::GeneralCellTransportInputV1 {
        geometry: &geometry,
        audit: &audit,
        source,
        schedule: &continuation,
        closure: &continuation_closure,
        positive_continuous: &continuation_positive,
        paper_thickness_mm: 0.1,
        tolerance: 1.0e-9,
        limits,
    };
    let flatten_work = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len() * cell.bottom_to_top_faces.len())
        .sum::<usize>()
        * (flatten_closure.leaves().len() + 1);
    let flatten_limits = crate::GeneralCellTransportLimitsV1 {
        max_transitions: flatten_closure.leaves().len() + 1,
        max_boundary_samples: flatten_work,
        ..cell_limits
    };
    let flatten_input = || crate::GeneralCellTransportInputV1 {
        geometry: &geometry,
        audit: &audit,
        source,
        schedule: &flatten,
        closure: &flatten_closure,
        positive_continuous: &flatten_positive,
        paper_thickness_mm: 0.1,
        tolerance: 1.0e-9,
        limits: flatten_limits,
    };
    let chained =
        crate::general_cell_transport::ChainedGeneralCellTransportAuthorityV1::issue(vec![
            first_input(),
            continuation_input(continuation_limits),
        ])
        .unwrap();
    assert_eq!(chained.proofs().len(), 2);
    let target_binding = [0x31; 32];
    let path_binding = [0x73; 32];
    let petal = crate::general_cell_transport::RegularQuadPetalPrivateRecordV1::issue(
        41,
        7,
        target_binding,
        path_binding,
        vec![
            first_input(),
            continuation_input(continuation_limits),
            flatten_input(),
        ],
    )
    .unwrap();
    assert!(petal.revalidates_for_apply_v1(41, 7, target_binding, path_binding));
    assert!(!petal.revalidates_for_apply_v1(42, 7, target_binding, path_binding));
    assert!(!petal.revalidates_for_apply_v1(41, 8, target_binding, path_binding));
    assert!(!petal.revalidates_for_apply_v1(41, 7, [0; 32], path_binding));
    assert!(!petal.revalidates_for_apply_v1(41, 7, target_binding, [0; 32]));
    let reversed_stationary =
        crate::general_cell_transport::ChainedGeneralCellTransportAuthorityV1::issue(vec![
            continuation_input(continuation_limits),
            first_input(),
        ])
        .expect("stationary identity transports are order-invariant");
    assert_eq!(reversed_stationary.proofs().len(), 2);
    assert!(matches!(
        crate::general_cell_transport::ChainedGeneralCellTransportAuthorityV1::issue(vec![
            first_input(),
            continuation_input(crate::GeneralCellTransportLimitsV1 {
                max_transitions: 0,
                ..continuation_limits
            }),
        ]),
        Err(crate::GeneralCellTransportErrorV1::ResourceLimit)
    ));
    assert!(matches!(
        crate::general_cell_transport::RegularQuadPetalPrivateRecordV1::issue(
            41,
            7,
            target_binding,
            path_binding,
            vec![
                first_input(),
                continuation_input(crate::GeneralCellTransportLimitsV1 {
                    max_transitions: 0,
                    ..continuation_limits
                }),
                flatten_input(),
            ],
        ),
        Err(crate::GeneralCellTransportErrorV1::ResourceLimit)
    ));
    assert_eq!(
        bound_cell_proof.checkpoint_hashes().len(),
        closure.leaves().len() + 1,
        "every certified closure checkpoint must carry all-cell transport evidence"
    );
    assert_eq!(
        bound_cell_proof.pair_order_count(),
        source.face_pair_orders.len()
    );
    assert!(!bound_cell_proof.is_for(
        &geometry,
        source,
        &schedule,
        &closure,
        f64::from_bits(0.1_f64.to_bits() + 1),
    ));
    let mut tampered_source = source.clone();
    tampered_source.provenance.source.source_revision += 1;
    assert!(!bound_cell_proof.is_for(&geometry, &tampered_source, &schedule, &closure, 0.1));
    assert!(matches!(
        crate::certify_general_multi_face_cell_transport_v1(crate::GeneralCellTransportInputV1 {
            geometry: &geometry,
            audit: &audit,
            source,
            schedule: &schedule,
            closure: &closure,
            positive_continuous: &bound_certificate,
            paper_thickness_mm: 10_000.0,
            tolerance: 1.0e-9,
            limits: cell_limits,
        },),
        Err(crate::GeneralCellTransportErrorV1::BindingMismatch)
    ));
    let initial_pose = geometry
        .solve_closed(&audit, fixed, &schedule.evaluate(0.0).unwrap(), 1.0e-9)
        .unwrap();
    assert!(matches!(
        prove_positive_thickness_graph_geometry_v1(
            &geometry,
            &initial_pose,
            0.1,
            PositiveThicknessGraphLimitsV1 {
                max_unordered_face_pairs: expected_pairs - 1,
                ..PositiveThicknessGraphLimitsV1::default()
            },
        ),
        Err(crate::PositiveThicknessGraphProofErrorV1::ResourceLimit)
    ));
    let foreign_geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    assert!(!bound_cell_proof.is_for(&foreign_geometry, source, &schedule, &closure, 0.1));
    assert!(!bound_certificate.is_for(&foreign_geometry, &audit, fixed, &schedule, &closure, 0.1,));
    assert!(
        diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &foreign_geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            1,
        )
        .continuous_certificate_model_id()
        .is_none()
    );
}

#[test]
fn miura_rank_eight_to_sixty_four_stationary_cell_proofs_are_bounded() {
    for (columns, rows, rank) in [(3, 5, 8usize), (5, 5, 16), (5, 9, 32), (9, 9, 64)] {
        let (pattern, paper, horizontal, _) =
            super::dense_grid_cycle_test_support::miura_authority_pattern(columns, rows);
        let project = ProjectId::new();
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: project,
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .unwrap();
        let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
        let global = ori_foldability::analyze_global_flat_foldability(
            ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
                project, &paper, &pattern, &topology, &local,
            ),
            ori_foldability::GlobalFlatFoldabilityLimits::default(),
        )
        .unwrap();
        let source = global.layer_order().expect("Miura global authority");
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), rank);
        let fixed = geometry.face_ids()[0];
        let schedule_limits = CycleScheduleLimitsV1 {
            max_hinges: geometry.hinges().len(),
            ..CycleScheduleLimitsV1::default()
        };
        let active = horizontal.into_iter().take(columns).collect::<HashSet<_>>();
        let mut entries = geometry
            .hinges()
            .iter()
            .map(|hinge| HalfAngleRationalEntryInputV1 {
                edge: hinge.edge(),
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: if active.contains(&hinge.edge()) {
                        100
                    } else {
                        1
                    },
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            entries,
            schedule_limits,
        )
        .unwrap();
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 8,
                    max_leaves: 256,
                    max_work: 1_000_000,
                    schedule_limits,
                },
            )
            .unwrap();
        let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry, &audit, fixed, &schedule, &closure, 0.1, 1,
        )
        .unwrap();
        let transitions = closure.leaves().len() + 1;
        let layer_records = source
            .overlap_cells
            .iter()
            .map(|cell| cell.bottom_to_top_faces.len())
            .sum::<usize>();
        let boundary_samples = source
            .overlap_cells
            .iter()
            .map(|cell| cell.exact_boundary.len() * cell.bottom_to_top_faces.len())
            .sum::<usize>()
            * transitions;
        let limits = crate::GeneralCellTransportLimitsV1 {
            max_transitions: transitions,
            max_cells: source.overlap_cells.len(),
            max_layer_records: layer_records,
            max_boundary_samples: boundary_samples,
        };
        let certify = || {
            crate::certify_general_multi_face_cell_transport_v1(
                crate::GeneralCellTransportInputV1 {
                    geometry: &geometry,
                    audit: &audit,
                    source,
                    schedule: &schedule,
                    closure: &closure,
                    positive_continuous: &positive,
                    paper_thickness_mm: 0.1,
                    tolerance: 1.0e-9,
                    limits,
                },
            )
        };
        let first = certify().unwrap();
        let second = certify().unwrap();
        assert_eq!(first.checkpoint_hashes(), second.checkpoint_hashes());
        assert_eq!(first.checkpoint_hashes().len(), transitions);
        let mut reordered = source.clone();
        reordered.overlap_cells.reverse();
        reordered.folded_faces.reverse();
        let reordered_proof = crate::certify_general_multi_face_cell_transport_v1(
            crate::GeneralCellTransportInputV1 {
                geometry: &geometry,
                audit: &audit,
                source: &reordered,
                schedule: &schedule,
                closure: &closure,
                positive_continuous: &positive,
                paper_thickness_mm: 0.1,
                tolerance: 1.0e-9,
                limits,
            },
        )
        .unwrap();
        assert_eq!(
            first.checkpoint_hashes(),
            reordered_proof.checkpoint_hashes()
        );
        assert!(matches!(
            crate::certify_general_multi_face_cell_transport_v1(
                crate::GeneralCellTransportInputV1 {
                    limits: crate::GeneralCellTransportLimitsV1 {
                        max_boundary_samples: boundary_samples - 1,
                        ..limits
                    },
                    geometry: &geometry,
                    audit: &audit,
                    source,
                    schedule: &schedule,
                    closure: &closure,
                    positive_continuous: &positive,
                    paper_thickness_mm: 0.1,
                    tolerance: 1.0e-9,
                },
            ),
            Err(crate::GeneralCellTransportErrorV1::ResourceLimit)
        ));
    }
}
