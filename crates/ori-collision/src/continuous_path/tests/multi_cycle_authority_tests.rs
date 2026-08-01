//! Regression coverage for the native four-through-eight-cycle cactus path. This remains
//! deliberately separate from desktop post-Apply proof tests: the authority
//! here is issued directly by the collision/kinematics boundary.

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use ori_kinematics::{CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;

fn fixture_planar_cross_v1(
    first: ori_domain::Point2,
    second: ori_domain::Point2,
    third: ori_domain::Point2,
) -> f64 {
    (second.x - first.x) * (third.y - first.y) - (second.y - first.y) * (third.x - first.x)
}

fn fixture_point_on_segment_v1(
    start: ori_domain::Point2,
    end: ori_domain::Point2,
    point: ori_domain::Point2,
) -> bool {
    fixture_planar_cross_v1(start, end, point) == 0.0
        && point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn fixture_segments_intersect_v1(
    first_start: ori_domain::Point2,
    first_end: ori_domain::Point2,
    second_start: ori_domain::Point2,
    second_end: ori_domain::Point2,
) -> bool {
    let first_side = fixture_planar_cross_v1(first_start, first_end, second_start);
    let second_side = fixture_planar_cross_v1(first_start, first_end, second_end);
    let third_side = fixture_planar_cross_v1(second_start, second_end, first_start);
    let fourth_side = fixture_planar_cross_v1(second_start, second_end, first_end);
    first_side * second_side < 0.0 && third_side * fourth_side < 0.0
        || fixture_point_on_segment_v1(first_start, first_end, second_start)
        || fixture_point_on_segment_v1(first_start, first_end, second_end)
        || fixture_point_on_segment_v1(second_start, second_end, first_start)
        || fixture_point_on_segment_v1(second_start, second_end, first_end)
}

fn separated_bifold_authority_fixture_v1(
    block_count: usize,
) -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    ori_kinematics::CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
    FaceId,
) {
    let (pattern, paper, moving) = if block_count == 10 {
        super::super::four_bay_cycle_test_support::ten_bay_opposite_bifold_pattern()
    } else {
        super::super::four_bay_cycle_test_support::bounded_bay_opposite_bifold_pattern(block_count)
    };
    let analysis = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b601", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    let topology = analysis.snapshot.unwrap_or_else(|| {
        panic!(
            "{block_count} separated opposite-bifold topology: {:?}; paper={:?}",
            analysis.issues,
            ori_core::validate_paper(&paper, &pattern).issues,
        )
    });
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("separated opposite-bifold geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("separated opposite-bifold audit");
    let fixed = topology
        .faces
        .iter()
        .max_by_key(|face| {
            topology
                .hinge_adjacency
                .iter()
                .filter(|hinge| hinge.first == face.id || hinge.second == face.id)
                .count()
        })
        .expect("shared exterior articulation face")
        .id;
    let moving = moving.into_iter().collect::<std::collections::HashSet<_>>();
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let active = moving.contains(&hinge.edge());
            ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge: hinge.edge(),
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
                numerator_power_coefficients: if active {
                    vec![
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ]
                } else {
                    vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                    numerator: if active { 100 } else { 1 },
                    denominator: 1,
                }],
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("separated opposite-bifold schedule");
    let bounded_block_closure_limits =
        |max_depth, max_leaves, max_work| DyadicIntervalClosureLimitsV1 {
            max_depth,
            max_leaves,
            max_work,
            schedule_limits: CycleScheduleLimitsV1::default(),
        };
    let exact_closure_limits = match block_count {
        8 => Some((3, 8, 8)),
        9 => Some((4, 9, 9)),
        10 => Some((4, 10, 10)),
        _ => None,
    };
    if let Some((max_depth, max_leaves, max_work)) = exact_closure_limits {
        for limits in [
            bounded_block_closure_limits(max_depth - 1, max_leaves, max_work),
            bounded_block_closure_limits(max_depth, max_leaves - 1, max_work),
            bounded_block_closure_limits(max_depth, max_leaves, max_work - 1),
        ] {
            assert!(matches!(
                geometry
                    .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, limits,),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
            ));
        }
    }
    let closure_limits = if let Some((max_depth, max_leaves, max_work)) = exact_closure_limits {
        bounded_block_closure_limits(max_depth, max_leaves, max_work)
    } else {
        DyadicIntervalClosureLimitsV1 {
            max_depth: 8,
            max_leaves: 256,
            max_work: 1_000_000,
            schedule_limits: CycleScheduleLimitsV1::default(),
        }
    };
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, closure_limits)
        .unwrap_or_else(|error| {
            panic!("{block_count}-block separated opposite-bifold closure: {error:?}")
        });
    if let Some((_, max_leaves, _)) = exact_closure_limits {
        assert_eq!(closure.leaves().len(), max_leaves);
        let expected_partition = match block_count {
            8 => (0_u64..8).map(|index| (3, index)).collect::<Vec<_>>(),
            9 => (0_u64..7)
                .map(|index| (3, index))
                .chain([(4, 14), (4, 15)])
                .collect::<Vec<_>>(),
            10 => (0_u64..6)
                .map(|index| (3, index))
                .chain([(4, 12), (4, 13), (4, 14), (4, 15)])
                .collect::<Vec<_>>(),
            _ => unreachable!("exact closure limits are defined only for 8..=10 blocks"),
        };
        assert_eq!(
            closure
                .leaves()
                .iter()
                .map(|(depth, index, _)| (*depth, *index))
                .collect::<Vec<_>>(),
            expected_partition,
        );
        assert!(closure.has_canonical_complete_partition_v1());
        assert!(closure.every_leaf_covers_graph_v1(&geometry));
    }
    (geometry, audit, schedule, closure, fixed)
}

#[test]
fn four_bay_opposite_bifold_fixture_retains_original_bit_layout() {
    let (pattern, paper, moving) =
        super::super::four_bay_cycle_test_support::four_bay_opposite_bifold_pattern();
    let namespace: ori_domain::ProjectId =
        serde_json::from_str("\"00000000-0000-4000-b000-000000000006\"").unwrap();
    let centers = [(-20.0, -20.0), (20.0, -20.0), (20.0, 20.0), (-20.0, 20.0)];
    let directions = [
        [
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
        ],
        [
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
        ],
        [
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
        ],
        [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
        ],
    ];
    assert_eq!((pattern.vertices.len(), pattern.edges.len()), (28, 48));
    assert_eq!((paper.boundary_vertices.len(), moving.len()), (24, 8));
    for (group, ((center_x, center_y), group_directions)) in
        centers.into_iter().zip(directions).enumerate()
    {
        let center = &pattern.vertices[group * 7];
        assert_eq!(
            center.id,
            ori_domain::VertexId::derive_v5(namespace, &[0x10, group as u8])
        );
        assert_eq!(center.position, ori_domain::Point2::new(center_x, center_y));
        for (local, (x, y)) in group_directions.into_iter().enumerate() {
            let endpoint = &pattern.vertices[group * 7 + local + 1];
            let expected =
                ori_domain::VertexId::derive_v5(namespace, &[0x20, group as u8, local as u8]);
            assert_eq!(endpoint.id, expected);
            assert_eq!(
                endpoint.position,
                ori_domain::Point2::new(center_x + x, center_y + y)
            );
            assert_eq!(paper.boundary_vertices[group * 6 + local], expected);
        }
    }
    for index in 0..24 {
        let boundary = &pattern.edges[index];
        assert_eq!(
            boundary.id,
            ori_domain::EdgeId::derive_v5(namespace, &[0x50, index as u8])
        );
        assert_eq!(boundary.start, paper.boundary_vertices[index]);
        assert_eq!(boundary.end, paper.boundary_vertices[(index + 1) % 24]);
        assert_eq!(boundary.kind, ori_domain::EdgeKind::Boundary);
        let hinge = &pattern.edges[index + 24];
        let expected_hinge = ori_domain::EdgeId::derive_v5(namespace, &[0x60, index as u8]);
        assert_eq!(hinge.id, expected_hinge);
        assert_eq!(
            hinge.kind,
            if matches!(index % 6, 0 | 1 | 3 | 4) {
                ori_domain::EdgeKind::Mountain
            } else {
                ori_domain::EdgeKind::Valley
            }
        );
    }
    let expected_moving = (0..4)
        .flat_map(|group| {
            [
                ori_domain::EdgeId::derive_v5(namespace, &[0x60, (group * 6) as u8]),
                ori_domain::EdgeId::derive_v5(namespace, &[0x60, (group * 6 + 3) as u8]),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(moving, expected_moving);
}

#[test]
fn bounded_opposite_bifold_selector_rejects_ten_before_fixture_construction() {
    let panic = match std::panic::catch_unwind(|| {
        super::super::four_bay_cycle_test_support::bounded_bay_opposite_bifold_pattern(10)
    }) {
        Ok(_) => panic!("ten-bay selector call must fail before fixture construction"),
        Err(payload) => payload,
    };
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("selector panic message");
    assert_eq!(
        message,
        "unsupported opposite-bifold fixture arity: 10; expected 4..=9"
    );
}

fn assert_extended_opposite_bifold_fixture_v1(block_count: usize) {
    assert!(
        matches!(block_count, 5..=10),
        "unsupported extended opposite-bifold fixture arity: {block_count}; expected 5..=10"
    );
    let (pattern, paper, moving) = if block_count == 10 {
        super::super::four_bay_cycle_test_support::ten_bay_opposite_bifold_pattern()
    } else {
        super::super::four_bay_cycle_test_support::bounded_bay_opposite_bifold_pattern(block_count)
    };
    let validation = ori_core::validate_paper(&paper, &pattern);
    assert!(
        validation.is_valid(),
        "{block_count}-bay paper: {:?}",
        validation.issues
    );
    assert_eq!(
        (paper.boundary_vertices.len(), moving.len()),
        (block_count * 6, block_count * 2)
    );
    let position = |vertex| {
        pattern
            .vertices
            .iter()
            .find(|candidate| candidate.id == vertex)
            .expect("fixture vertex")
            .position
    };
    let boundary = paper
        .boundary_vertices
        .iter()
        .copied()
        .map(position)
        .collect::<Vec<_>>();
    for first in 0..boundary.len() {
        for second in first + 1..boundary.len() {
            if second == first + 1 || first == 0 && second + 1 == boundary.len() {
                continue;
            }
            assert!(
                !fixture_segments_intersect_v1(
                    boundary[first],
                    boundary[(first + 1) % boundary.len()],
                    boundary[second],
                    boundary[(second + 1) % boundary.len()],
                ),
                "non-adjacent boundary segments {first} and {second}",
            );
        }
    }

    let radial_hinges = &pattern.edges[paper.boundary_vertices.len()..];
    assert_eq!(radial_hinges.len(), block_count * 6);
    assert!(radial_hinges.chunks_exact(6).remainder().is_empty());
    let moving_set = moving
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let paper_thickness = paper.thickness_mm;
    assert_eq!(paper_thickness.to_bits(), 0.1_f64.to_bits());
    for (group, hinges) in radial_hinges.chunks_exact(6).enumerate() {
        assert!(hinges.iter().all(|hinge| hinge.start == hinges[0].start));
        assert_eq!(
            hinges
                .iter()
                .filter(|hinge| hinge.kind == ori_domain::EdgeKind::Mountain)
                .count(),
            4,
            "bay {group} Maekawa mountain count",
        );
        assert_eq!(
            hinges
                .iter()
                .filter(|hinge| hinge.kind == ori_domain::EdgeKind::Valley)
                .count(),
            2,
            "bay {group} Maekawa valley count",
        );
        for (local, hinge) in hinges.iter().enumerate() {
            assert_eq!(
                hinge.kind,
                if moving_set.contains(&hinge.id) {
                    ori_domain::EdgeKind::Valley
                } else {
                    ori_domain::EdgeKind::Mountain
                },
                "bay {group} layer-consistent assignment at ray {local}",
            );
        }
        let center = position(hinges[0].start);
        let directions = hinges
            .iter()
            .map(|hinge| {
                let endpoint = position(hinge.end);
                ori_domain::Point2::new(endpoint.x - center.x, endpoint.y - center.y)
            })
            .collect::<Vec<_>>();
        for (local, (hinge, direction)) in hinges.iter().zip(&directions).enumerate() {
            let length_squared = direction.x * direction.x + direction.y * direction.y;
            let short_corner = matches!(
                (block_count, group),
                (5, 0 | 3)
                    | (6, 0..=3)
                    | (7, 0..=3 | 6)
                    | (8, 0..=3 | 6 | 7)
                    | (9, 0..=4 | 6..=8)
                    | (10, 0..=4 | 6..=9)
            );
            if short_corner && moving_set.contains(&hinge.id) {
                assert!(
                    length_squared >= (5.0 * paper_thickness).powi(2)
                        && length_squared <= (6.0 * paper_thickness).powi(2),
                    "bay {group} corridor-bound moving ray {local}",
                );
            } else {
                assert!(
                    length_squared >= 1.0,
                    "bay {group} non-corridor ray {local} remains unit-or-longer",
                );
            }
        }
        for (first, opposite) in [(0, 3), (1, 4), (2, 5)] {
            assert_eq!(directions[first].x, -directions[opposite].x);
            assert_eq!(directions[first].y, -directions[opposite].y);
        }
        let angles = directions
            .iter()
            .map(|direction| direction.y.atan2(direction.x))
            .collect::<Vec<_>>();
        let sectors = (0..6)
            .map(|index| {
                (angles[(index + 1) % 6] - angles[index]).rem_euclid(std::f64::consts::TAU)
            })
            .collect::<Vec<_>>();
        assert!(sectors.iter().all(|sector| *sector > 0.0));
        let first_alternating = sectors[0] + sectors[2] + sectors[4];
        let second_alternating = sectors[1] + sectors[3] + sectors[5];
        assert!((first_alternating - std::f64::consts::PI).abs() < 1.0e-12);
        assert!((second_alternating - std::f64::consts::PI).abs() < 1.0e-12);
    }

    let analysis = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id(
            match block_count {
                5 => "b605",
                6 => "b606",
                7 => "b607",
                8 => "b608",
                9 => "b609",
                10 => "b610",
                _ => unreachable!(),
            },
            1,
        ),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    let topology = analysis
        .snapshot
        .unwrap_or_else(|| panic!("{block_count}-bay topology: {:?}", analysis.issues));
    assert_eq!(
        (topology.faces.len(), topology.hinge_adjacency.len()),
        (block_count * 5 + 1, block_count * 6)
    );
    let shared = topology
        .faces
        .iter()
        .max_by_key(|face| face.outer.half_edges.len())
        .expect("shared material face");
    assert_eq!(shared.outer.half_edges.len(), block_count * 3);
    let shared_polygon = shared
        .outer
        .half_edges
        .iter()
        .map(|half_edge| position(half_edge.origin))
        .collect::<Vec<_>>();
    let turns = (0..shared_polygon.len())
        .map(|index| {
            fixture_planar_cross_v1(
                shared_polygon[index],
                shared_polygon[(index + 1) % shared_polygon.len()],
                shared_polygon[(index + 2) % shared_polygon.len()],
            )
        })
        .collect::<Vec<_>>();
    let orientation = turns
        .iter()
        .copied()
        .find(|turn| *turn != 0.0)
        .expect("strict shared-face corner")
        .signum();
    assert!(
        turns.iter().all(|turn| *turn * orientation >= 0.0),
        "shared face is not convex: polygon={shared_polygon:?}, turns={turns:?}",
    );
    assert_eq!(
        turns.iter().filter(|turn| **turn != 0.0).count(),
        block_count
    );
}

#[test]
fn bounded_extended_opposite_bifold_fixtures_are_simple_convex_and_locally_flat_foldable() {
    for block_count in [5, 6, 7, 8] {
        assert_extended_opposite_bifold_fixture_v1(block_count);
    }
}

#[test]
fn exact_nine_extended_opposite_bifold_fixture_is_simple_convex_and_locally_flat_foldable() {
    assert_extended_opposite_bifold_fixture_v1(9);
}

#[test]
fn exact_ten_extended_opposite_bifold_fixture_is_simple_convex_and_locally_flat_foldable() {
    assert_extended_opposite_bifold_fixture_v1(10);
}

#[test]
fn bounded_separated_opposite_bifolds_issue_strict_parent_positive_authority() {
    for block_count in [4, 5, 6, 7, 8] {
        assert_separated_bifold_parent_positive_authority_v1(block_count);
    }
}

#[test]
fn exact_nine_separated_opposite_bifolds_issue_strict_parent_positive_authority() {
    assert_separated_bifold_parent_positive_authority_v1(9);
}

#[test]
fn exact_ten_separated_opposite_bifolds_issue_canonical_closure() {
    let (geometry, audit, schedule, closure, fixed) = separated_bifold_authority_fixture_v1(10);
    assert_eq!((geometry.face_ids().len(), geometry.hinges().len()), (51, 60));
    assert!(schedule.matches_binding(&geometry, &audit, fixed));
    assert_eq!(closure.leaves().len(), 10);
    assert!(closure.has_canonical_complete_partition_v1());
    assert!(closure.every_leaf_covers_graph_v1(&geometry));
}

fn assert_separated_bifold_parent_positive_authority_v1(block_count: usize) {
    let (geometry, audit, schedule, closure, fixed) =
        separated_bifold_authority_fixture_v1(block_count);
    assert_eq!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (block_count * 5 + 1, block_count * 6)
    );
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(&audit, CanonicalEdgeBlockLimitsV1::default())
        .unwrap_or_else(|_| panic!("{block_count} canonical radial-bifold blocks"));
    assert_eq!(decomposition.blocks().len(), block_count);
    assert_eq!(decomposition.articulation_faces(), &[fixed]);
    assert!(decomposition.blocks().iter().all(|block| {
        block.geometry().face_ids().len() == 6
            && block.geometry().hinges().len() == 6
            && block.geometry().face_ids().contains(&fixed)
    }));
    for (index, block) in decomposition.blocks().iter().enumerate() {
        let first_hinge = &block.geometry().hinges()[0];
        let pivot = [first_hinge.start(), first_hinge.end()]
            .into_iter()
            .find(|candidate| {
                block
                    .geometry()
                    .hinges()
                    .iter()
                    .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
            })
            .expect("radial pivot");
        let block_schedule = schedule
            .restrict_to_edge_block_with_fixed_face_v1(
                &geometry,
                &audit,
                block.geometry(),
                block.audit(),
                fixed,
            )
            .unwrap_or_else(|_| panic!("restrict {block_count}-block radial schedule {index}"));
        assert!(
            opposite_radial_bifold_group_bounds_v1(
                block.geometry(),
                block.audit(),
                fixed,
                &block_schedule,
                None,
            )
            .is_some(),
            "{block_count}-block structural radial theorem group {index} at {pivot:?}",
        );
        assert!(
            opposite_radial_bifold_group_bounds_v1(
                block.geometry(),
                block.audit(),
                fixed,
                &block_schedule,
                Some(0.1),
            )
            .is_some(),
            "{block_count}-block thickness radial theorem group {index} at {pivot:?}",
        );
    }
    assert!(
        scheduled_separated_common_articulation_bifolds_premises_v1(
            &geometry, &audit, fixed, &schedule, &closure, 0.1, None,
        ),
        "{block_count}-block separated radial parent premises",
    );

    let first_block = &decomposition.blocks()[0];
    let first_block_schedule = schedule
        .restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            first_block.geometry(),
            first_block.audit(),
            fixed,
        )
        .expect("restrict the first radial-bifold schedule");
    let block_moving = first_block_schedule
        .collective_profile_edges_v1()
        .expect("the genuine block uses one exact collective profile");
    assert_eq!(block_moving.len(), 2);
    assert!(
        opposite_radial_bifold_group_bounds_v1(
            first_block.geometry(),
            first_block.audit(),
            fixed,
            &first_block_schedule,
            Some(0.1),
        )
        .is_some(),
        "the half-angle fixture satisfies the complete-domain bifold theorem",
    );
    let coefficient = |numerator| ori_kinematics::RationalCoefficientV1 {
        numerator,
        denominator: 1,
    };
    let mut divergent_entries = first_block
        .geometry()
        .hinges()
        .iter()
        .map(|hinge| {
            let edge = hinge.edge();
            let numerator_power_coefficients = if edge == block_moving[0] {
                vec![coefficient(0), coefficient(1)]
            } else if edge == block_moving[1] {
                // u(2-u)/100 has the same exact endpoints as u/100 but a
                // different open-interval profile.
                vec![coefficient(0), coefficient(2), coefficient(-1)]
            } else {
                vec![coefficient(0)]
            };
            ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [coefficient(0), coefficient(1)],
                numerator_power_coefficients,
                denominator_power_coefficients: vec![coefficient(
                    if block_moving.contains(&edge) { 100 } else { 1 },
                )],
            }
        })
        .collect::<Vec<_>>();
    divergent_entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let divergent = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        first_block.geometry(),
        first_block.audit(),
        fixed,
        divergent_entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("same-endpoint divergent block schedule");
    assert_eq!(first_block_schedule.evaluate(0.0), divergent.evaluate(0.0));
    assert_eq!(first_block_schedule.evaluate(1.0), divergent.evaluate(1.0));
    assert!(divergent.collective_profile_edges_v1().is_none());
    assert!(
        opposite_radial_bifold_group_bounds_v1(
            first_block.geometry(),
            first_block.audit(),
            fixed,
            &divergent,
            None,
        )
        .is_none(),
        "endpoint equality must not impersonate one exact collective profile",
    );

    let block_moving_set = block_moving
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut long_domain_entries = first_block
        .geometry()
        .hinges()
        .iter()
        .map(|hinge| {
            let active = block_moving_set.contains(&hinge.edge());
            ori_kinematics::CycleScheduleEntryInputV1 {
                edge: hinge.edge(),
                initial_angle_degrees_bits: if active {
                    90.0_f64.to_bits()
                } else {
                    0.0_f64.to_bits()
                },
                chebyshev_coefficients: if active {
                    vec![coefficient(0), coefficient(90)]
                } else {
                    vec![coefficient(0)]
                },
            }
        })
        .collect::<Vec<_>>();
    long_domain_entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let long_domain = ori_kinematics::CanonicalCycleScheduleV1::prepare(
        first_block.geometry(),
        first_block.audit(),
        fixed,
        [0.0, 3.0],
        long_domain_entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("ordinary long-domain radial schedule");
    assert_eq!(
        long_domain.derivative_bound(block_moving[0]),
        Some(60.0),
        "the derivative bound alone is below ninety degrees",
    );
    assert!(
        long_domain
            .evaluate(3.0)
            .is_some_and(|angles| angles.as_slice().iter().all(|angle| {
                !block_moving_set.contains(&angle.edge()) || angle.angle_degrees() == 180.0
            })),
        "the complete domain nevertheless reaches a half turn",
    );
    assert!(
        opposite_radial_bifold_group_bounds_v1(
            first_block.geometry(),
            first_block.audit(),
            fixed,
            &long_domain,
            Some(0.1),
        )
        .is_none(),
        "the full-domain angle enclosure must reject the derivative-only impersonator",
    );

    for progress in [0.0, 0.5, 1.0] {
        let angles = schedule
            .evaluate(progress)
            .expect("bounded bifold schedule sample");
        let pose = geometry
            .solve_closed(&audit, fixed, &angles, 1.0e-9)
            .expect("closed bounded bifold sample");
        if progress == 0.0 {
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1::default(),
            )
            .expect("initial positive-thickness bifold geometry");
        }
    }

    let authority = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry, &audit, fixed, &schedule, &closure, 0.1, 32,
    )
    .unwrap_or_else(|| panic!("separated {block_count}-block parent positive authority"));
    assert!(authority.is_for(&geometry, &audit, fixed, &schedule, &closure, 0.1,));
    assert!(!authority.is_for(
        &geometry,
        &audit,
        fixed,
        &schedule,
        &closure,
        f64::from_bits(0.1_f64.to_bits() + 1),
    ));
    let foreign_fixed = geometry
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != fixed)
        .expect("a non-articulation face");
    assert!(!authority.is_for(&geometry, &audit, foreign_fixed, &schedule, &closure, 0.1,));
}

#[test]
fn radial_bifold_theorem_rejects_structural_and_separation_impersonators() {
    let (geometry, audit, schedule, closure, fixed) = separated_bifold_authority_fixture_v1(4);
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(&audit, CanonicalEdgeBlockLimitsV1::default())
        .expect("four canonical radial-bifold blocks");
    let first_block = &decomposition.blocks()[0];
    let first_block_schedule = schedule
        .restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            first_block.geometry(),
            first_block.audit(),
            fixed,
        )
        .expect("restrict the first radial-bifold schedule");
    let block_moving = first_block_schedule
        .collective_profile_edges_v1()
        .expect("the genuine block uses one exact collective profile");
    let reference_hinge = first_block
        .geometry()
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == block_moving[0])
        .expect("first genuine moving hinge");
    let same_assignment_nonopposite = first_block
        .geometry()
        .hinges()
        .iter()
        .find(|hinge| {
            !block_moving.contains(&hinge.edge())
                && hinge.assignment() == reference_hinge.assignment()
        })
        .expect("inactive same-assignment nonopposite ray")
        .edge();
    let different_assignment = first_block
        .geometry()
        .hinges()
        .iter()
        .find(|hinge| hinge.assignment() != reference_hinge.assignment())
        .expect("inactive different-assignment ray")
        .edge();
    let coefficient = |numerator| ori_kinematics::RationalCoefficientV1 {
        numerator,
        denominator: 1,
    };
    let prepare_candidate = |active_edges: [EdgeId; 2]| {
        let mut entries = first_block
            .geometry()
            .hinges()
            .iter()
            .map(|hinge| {
                let active = active_edges.contains(&hinge.edge());
                ori_kinematics::HalfAngleRationalEntryInputV1 {
                    edge: hinge.edge(),
                    u_domain: [coefficient(0), coefficient(1)],
                    numerator_power_coefficients: if active {
                        vec![coefficient(0), coefficient(1)]
                    } else {
                        vec![coefficient(0)]
                    },
                    denominator_power_coefficients: vec![coefficient(if active { 100 } else { 1 })],
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
            first_block.geometry(),
            first_block.audit(),
            fixed,
            entries,
            CycleScheduleLimitsV1::default(),
        )
        .expect("bounded collective radial candidate")
    };

    for (candidate, reason) in [
        (
            prepare_candidate([block_moving[0], same_assignment_nonopposite]),
            "same-assignment nonopposite rays",
        ),
        (
            prepare_candidate([block_moving[0], different_assignment]),
            "different-assignment rays",
        ),
    ] {
        assert_eq!(
            candidate
                .collective_profile_edges_v1()
                .map(|edges| edges.len()),
            Some(2),
            "{reason} remain an exact collective-profile impersonator",
        );
        assert!(
            opposite_radial_bifold_group_bounds_v1(
                first_block.geometry(),
                first_block.audit(),
                fixed,
                &candidate,
                Some(0.1),
            )
            .is_none(),
            "the radial theorem must reject {reason}",
        );
    }

    assert!(scheduled_separated_common_articulation_bifolds_premises_v1(
        &geometry, &audit, fixed, &schedule, &closure, 0.1, None,
    ));
    for invalid_thickness in [0.0, -0.0, -0.1, f64::NAN, f64::INFINITY, 40.0] {
        assert!(
            !scheduled_separated_common_articulation_bifolds_premises_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                invalid_thickness,
                None,
            ),
            "invalid or nonseparated thickness {invalid_thickness:?} must fail closed",
        );
    }
}

fn four_cycle_geometry_at_revision_v1(
    revision: u64,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit, FaceId) {
    let (pattern, paper, _) =
        super::super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern();
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b600", 1),
        source_revision: revision,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("four-cycle rational cactus topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("revision-bound geometry");
    let fixed = topology
        .faces
        .iter()
        .max_by_key(|face| {
            topology
                .hinge_adjacency
                .iter()
                .filter(|hinge| hinge.first == face.id || hinge.second == face.id)
                .count()
        })
        .expect("articulation face")
        .id;
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("revision-bound graph audit");
    (geometry, audit, fixed)
}

fn stationary_zero_schedule_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed: FaceId,
) -> ori_kinematics::CanonicalCycleScheduleV1 {
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| ori_kinematics::CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: 0.0_f64.to_bits(),
            chebyshev_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    ori_kinematics::CanonicalCycleScheduleV1::prepare(
        geometry,
        audit,
        fixed,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("canonical stationary schedule")
}

fn stationary_four_cycle_authority_fixture_v1() -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    ori_kinematics::CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
    FaceId,
) {
    let (geometry, audit, _, fixed) = rational_cycle_bay_geometry(4, false);
    let schedule = stationary_zero_schedule_v1(&geometry, &audit, fixed);
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
        .expect("single-leaf stationary multi-cycle closure");
    (geometry, audit, schedule, closure, fixed)
}

#[test]
fn controlled_group_mid_stop_never_mints_or_contaminates_authority() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("four-leaf closure");

    let stop = configure_controlled_closure_leaf_test_stop_v1(
        2,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
    ));
    drop(stop);

    let stop = configure_controlled_closure_leaf_test_stop_v1(
        3,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
    ));
    drop(stop);

    let stop = configure_controlled_group_test_stop_v1(
        3,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
    ));
    drop(stop);

    let stop = configure_controlled_group_test_stop_v1(
        4,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
    ));
    drop(stop);

    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        )
        .expect("clean control context")
        .is_none(),
        "group separation and static samples alone must not mint continuous authority"
    );
}

#[test]
fn four_cycle_positive_authority_rejects_foreign_revision_cancelled_and_aba_contexts() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    assert_eq!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (13, 16)
    );
    assert_eq!(
        geometry.hinges().len() + 1 - geometry.face_ids().len(),
        4,
        "fixture must exercise four independent cycle closures"
    );
    assert_eq!(audit.closure_hinges().len(), 4);
    let cycle_groups =
        composed_symmetric_rational_local_groups_v1(&geometry, &audit, fixed, &schedule)
            .expect("four individually recognised symmetric cycle groups");
    let mut group_sizes = cycle_groups
        .values()
        .copied()
        .fold([0_usize; 4], |mut sizes, group| {
            sizes[group] += 1;
            sizes
        });
    group_sizes.sort_unstable();
    assert_eq!(group_sizes, [3, 3, 3, 3]);
    let bounded_start = Instant::now();
    assert!(
        matches!(
            geometry.prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 3,
                    max_work: 3,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            ),
            Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
        ),
        "one-short multi-cycle path budget must fail before an authority can be issued"
    );
    assert!(
        bounded_start.elapsed() < Duration::from_secs(2),
        "one-short multi-cycle closure must fail within the worker budget"
    );
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("four-leaf multi-cycle closure");
    assert_eq!(closure.leaves().len(), 4);
    let control_start = Instant::now();
    let active = AtomicBool::new(false);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &crate::CooperativeOperationControlV1::new(Some(&active), Instant::now()),
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
        ),
        "an elapsed deadline must stop the multi-cycle issuer before authority minting"
    );
    let cancelled = AtomicBool::new(true);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &crate::CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
        ),
        "a cancellation observed while issuing must leave no partial authority"
    );
    let generation = AtomicU64::new(41);
    let old_generation = crate::CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &generation,
        41,
        Instant::now() + Duration::from_secs(1),
    );
    generation.store(42, Ordering::Release);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &old_generation,
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
        ),
        "an old generation cannot publish after a replacement begins"
    );
    let current_generation = crate::CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &generation,
        42,
        Instant::now() + Duration::from_secs(1),
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &current_generation,
        )
        .expect("current generation control")
        .is_none(),
        "finite cactus samples and local group separation are not a continuous proof"
    );
    assert!(
        control_start.elapsed() < Duration::from_secs(2),
        "deadline, cancellation, and ABA issuance checks must remain bounded"
    );

    let (stationary, stationary_audit, stationary_schedule, stationary_closure, stationary_fixed) =
        stationary_four_cycle_authority_fixture_v1();
    let authority = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &stationary,
        &stationary_audit,
        stationary_fixed,
        &stationary_schedule,
        &stationary_closure,
        0.1,
        1,
    )
    .expect("stationary all-pair positive-thickness authority");
    assert!(authority.is_for(
        &stationary,
        &stationary_audit,
        stationary_fixed,
        &stationary_schedule,
        &stationary_closure,
        0.1
    ));

    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
            0,
        )
        .is_none(),
        "a zero proof-leaf budget cannot mint authority"
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
            MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1,
        )
        .is_none(),
        "an excessive proof-leaf budget cannot mint authority"
    );

    let wrong_fixed = stationary
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != stationary_fixed)
        .expect("another face");
    assert!(
        !authority.is_for(
            &stationary,
            &stationary_audit,
            wrong_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "the authority remains fixed-face bound"
    );
    assert!(
        !authority.is_for(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            f64::from_bits(0.1_f64.to_bits() + 1),
        ),
        "the authority remains binary64-thickness bound"
    );

    let (detached, detached_audit, detached_schedule, detached_closure, detached_fixed) =
        stationary_four_cycle_authority_fixture_v1();
    assert!(!stationary.same_instance(&detached));
    assert_eq!(
        stationary_schedule.certificate_binding_fingerprint_v2(),
        detached_schedule.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        stationary_closure.partition_binding_fingerprint_v2(),
        detached_closure.partition_binding_fingerprint_v2()
    );
    assert!(
        !authority.is_for(
            &stationary,
            &detached_audit,
            stationary_fixed,
            &detached_schedule,
            &detached_closure,
            0.1,
        ),
        "same-content foreign closure evidence must fail the issuer check"
    );
    assert!(
        !authority.is_for(
            &detached,
            &detached_audit,
            detached_fixed,
            &detached_schedule,
            &detached_closure,
            0.1,
        ),
        "same-content replacement geometry must fail the issuer check"
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &detached_closure,
            0.1,
            1,
        )
        .is_none(),
        "a same-content foreign closure cannot mint replacement authority"
    );

    let (revised, revised_audit, revised_fixed) = four_cycle_geometry_at_revision_v1(2);
    assert!(
        !authority.is_for(
            &revised,
            &revised_audit,
            revised_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "a new source revision must not reuse the old issuer authority"
    );

    assert!(
        authority.is_for(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "failed control contexts neither mint a replacement authority nor mutate the issued scene evidence"
    );
}
