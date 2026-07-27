use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3, TreeHinge,
    graph::{
        block_cut_carrier_free_product::block_cut_carrier_free_product_cycle_closure_premises_v1,
        block_cut_coaxial::block_cut_coaxial_cycle_closure_premises_v1,
        block_cut_decomposition::prepare_contracted_block_cut_v1,
        block_cut_free_word::block_cut_free_word_cycle_closure_premises_v1,
        block_cut_generalized_dihedral::block_cut_generalized_dihedral_cycle_closure_premises_v1,
        bridge_motion::bridge_only_motion_cycle_closure_premises_v1,
        coaxial_profile_lattice::coaxial_profile_lattice_cycle_closure_premises_v1,
        collective_flat_stack_cycle_closure_premises_v1,
        dense_grid::dense_parallel_grid_cycle_closure_premises_v1,
        even_single_vertex_opposite_pair_cycle_closure_premises_v1,
        exact_cut_carrier::exact_cut_carrier_cycle_closure_premises_v1,
        exact_generator_word::exact_generator_word_cycle_closure_premises_v1,
        orthogonal_inverse_pair_cycle_closure_premises_v1,
        symmetric_rational_kawasaki_cycle_closure_premises_v1,
        theta_opposite_pair_cycle_closure_premises_v1,
    },
};

fn with_constant_bridge_v1(fixture: OrthogonalFixtureV1, angle_bits: u64) -> OrthogonalFixtureV1 {
    let namespace = ProjectId::new();
    let leaf = FaceId::derive_v5(namespace, b"orthogonal-strict-bridge-face");
    let edge = EdgeId::derive_v5(namespace, b"orthogonal-strict-bridge-edge");
    let mut faces = fixture.geometry.face_ids().to_vec();
    faces.push(leaf);
    let mut hinges = fixture.geometry.hinges().to_vec();
    hinges.push(TreeHinge::new_for_test(
        edge,
        FoldAssignment::Mountain,
        fixture.cycle_faces[0],
        leaf,
        Point3::new(50.0, 0.0, 0.0).unwrap(),
        Point3::new(50.0, 1.0, 0.0).unwrap(),
        Point3::new(0.0, 1.0, 0.0).unwrap(),
    ));
    let mut profiles = fixture.profiles;
    profiles.push((edge, TestProfileV1::Constant(angle_bits)));
    rebuild_fixture_v1(
        faces,
        hinges,
        OrthogonalFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles,
            cycle_edges: fixture.cycle_edges,
            cycle_faces: fixture.cycle_faces,
            bridge_edges: vec![edge],
        },
        false,
    )
}

#[test]
fn orthogonal_half_turn_strictly_certifies_three_carrier_semidirect_cycle() {
    let overlapping_core = semidirect_fixture_v1(false, false, 0, false);
    let overlapping_schedule = polynomial_schedule_v1(&overlapping_core, ScheduleMutationV1::None);
    assert!(collective_flat_stack_cycle_closure_premises_v1(
        &overlapping_core.geometry,
        &overlapping_core.audit,
        overlapping_core.fixed_face,
        &overlapping_schedule,
        0.0,
    ));

    // The exact 37-degree bridge belongs to no closed walk, so it cannot
    // alter the new block-local proof. It deliberately lies outside the old
    // flat-stack issuer's requirement that every stationary hinge be 180
    // degrees, making this a strict new-family positive.
    let fixture = with_constant_bridge_v1(
        semidirect_fixture_v1(false, false, 0, false),
        37.0_f64.to_bits(),
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert_eq!(fixture.geometry.face_ids().len(), 6);
    assert_eq!(fixture.geometry.hinges().len(), 6);
    assert_eq!(schedule.collective_profile_edges_v1().unwrap().len(), 2);
    assert!(block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    type ExistingIssuerV1 = fn(
        &MaterialHingeGraphGeometry,
        &MaterialHingeGraphAudit,
        FaceId,
        &CanonicalCycleScheduleV1,
        f64,
    ) -> bool;
    let existing_issuers: [(&str, ExistingIssuerV1); 14] = [
        (
            "bridge_only_motion",
            bridge_only_motion_cycle_closure_premises_v1,
        ),
        (
            "dense_parallel_grid",
            dense_parallel_grid_cycle_closure_premises_v1,
        ),
        (
            "exact_cut_carrier",
            exact_cut_carrier_cycle_closure_premises_v1,
        ),
        (
            "exact_generator_word",
            exact_generator_word_cycle_closure_premises_v1,
        ),
        (
            "coaxial_profile_lattice",
            coaxial_profile_lattice_cycle_closure_premises_v1,
        ),
        (
            "block_cut_coaxial",
            block_cut_coaxial_cycle_closure_premises_v1,
        ),
        (
            "block_cut_free_word",
            block_cut_free_word_cycle_closure_premises_v1,
        ),
        (
            "block_cut_carrier_free_product",
            block_cut_carrier_free_product_cycle_closure_premises_v1,
        ),
        (
            "block_cut_generalized_dihedral",
            block_cut_generalized_dihedral_cycle_closure_premises_v1,
        ),
        (
            "collective_flat_stack",
            collective_flat_stack_cycle_closure_premises_v1,
        ),
        (
            "even_single_vertex_opposite_pair",
            even_single_vertex_opposite_pair_cycle_closure_premises_v1,
        ),
        (
            "orthogonal_inverse_pair",
            orthogonal_inverse_pair_cycle_closure_premises_v1,
        ),
        (
            "theta_opposite_pair",
            theta_opposite_pair_cycle_closure_premises_v1,
        ),
        (
            "symmetric_rational_kawasaki",
            symmetric_rational_kawasaki_cycle_closure_premises_v1,
        ),
    ];
    for (name, issuer) in existing_issuers {
        assert!(
            !issuer(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            ),
            "existing issuer {name} unexpectedly accepted the strict fixture",
        );
    }

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
        .expect("the exact orthogonal-half-turn issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn orthogonal_half_turn_accepts_one_two_and_three_carrier_finite_relations() {
    for fixture in [
        half_turn_square_fixture_v1(1),
        half_turn_square_fixture_v1(2),
        triangle_fixture_v1(false, false),
    ] {
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
        let angles = schedule.evaluate(0.5).unwrap();
        assert!(
            fixture
                .geometry
                .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-8)
                .is_ok()
        );
    }
}

#[test]
fn orthogonal_half_turn_contracts_exact_zero_and_leaves_bridges_relation_free() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let namespace = ProjectId::new();
    let zero_leaf = FaceId::derive_v5(namespace, b"orthogonal-zero-leaf");
    let bridge_leaf = FaceId::derive_v5(namespace, b"orthogonal-active-bridge-leaf");
    let zero_edge = EdgeId::derive_v5(namespace, b"orthogonal-zero-edge");
    let bridge_edge = EdgeId::derive_v5(namespace, b"orthogonal-active-bridge-edge");
    let mut faces = fixture.geometry.face_ids().to_vec();
    faces.extend([zero_leaf, bridge_leaf]);
    let mut hinges = fixture.geometry.hinges().to_vec();
    hinges.push(TreeHinge::new_for_test(
        zero_edge,
        FoldAssignment::Mountain,
        fixture.cycle_faces[0],
        zero_leaf,
        Point3::new(30.0, 0.0, 0.0).unwrap(),
        Point3::new(30.0, 1.0, 0.0).unwrap(),
        Point3::new(0.0, 1.0, 0.0).unwrap(),
    ));
    hinges.push(TreeHinge::new_for_test(
        bridge_edge,
        FoldAssignment::Mountain,
        fixture.cycle_faces[1],
        bridge_leaf,
        Point3::new(40.0, 0.0, 0.0).unwrap(),
        Point3::new(40.0, 0.0, 1.0).unwrap(),
        Point3::new(0.0, 0.0, 1.0).unwrap(),
    ));
    let mut profiles = fixture.profiles;
    profiles.extend([
        (zero_edge, TestProfileV1::Constant(0.0_f64.to_bits())),
        (bridge_edge, TestProfileV1::Constant(37.0_f64.to_bits())),
    ]);
    let fixture = rebuild_fixture_v1(
        faces,
        hinges,
        OrthogonalFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles,
            cycle_edges: fixture.cycle_edges,
            cycle_faces: fixture.cycle_faces,
            bridge_edges: vec![bridge_edge],
        },
        true,
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_is_storage_root_and_orientation_invariant() {
    for fixed_face in 0..5 {
        for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
            let fixture =
                semidirect_fixture_v1(reverse_every_other, reverse_storage, fixed_face, false);
            let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
            assert!(block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            ));
            for parameter in [0.0, 0.25, 0.5, 1.0] {
                let angles = schedule.evaluate(parameter).unwrap();
                assert!(
                    fixture
                        .geometry
                        .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-8)
                        .is_ok()
                );
            }
        }
    }
}

#[test]
fn twisted_reflection_is_independent_of_axis_orientation_and_half_turn_sign() {
    let fixture = triangle_fixture_v1(false, false);
    let valley_edge = fixture.cycle_edges[1];
    let reversed_axis_edge = fixture.cycle_edges[2];
    let fixture = replace_hinges_v1(fixture, &[valley_edge], |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            FoldAssignment::Valley,
            hinge.left_face(),
            hinge.right_face(),
            hinge.start(),
            hinge.end(),
            hinge.axis(),
        )
    });
    let fixture = replace_hinges_v1(fixture, &[reversed_axis_edge], |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            FoldAssignment::Mountain,
            hinge.left_face(),
            hinge.right_face(),
            hinge.end(),
            hinge.start(),
            Point3::new(-hinge.axis().x(), -hinge.axis().y(), -hinge.axis().z()).unwrap(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let angles = schedule.evaluate(0.5).unwrap();
    assert!(
        fixture
            .geometry
            .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-8)
            .is_ok()
    );
}

#[test]
fn exact_primary_half_turn_is_not_duplicated_as_a_free_profile() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    let (prepared, _, _, _) =
        preparation::prepare_orthogonal_half_turn_blocks_v1(&schedule, &decomposition).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].schema.profile_count, 1);
    assert!(prepared[0].has_primary_half_turn_label);
    assert!(
        prepared[0]
            .labels
            .iter()
            .any(|label| label.kind == preparation::PreparedOrthogonalEdgeKindV1::PrimaryHalfTurn)
    );
}

#[test]
fn nonconstant_profile_that_reaches_180_remains_a_conservative_generator() {
    let mut fixture = half_turn_square_fixture_v1(1);
    for edge in [fixture.cycle_edges[0], fixture.cycle_edges[1]] {
        fixture = replace_profile_v1(fixture, edge, TestProfileV1::Collective);
    }
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::CollectiveTouchesHalfTurn);
    assert_eq!(
        schedule
            .evaluate(1.0)
            .unwrap()
            .as_slice()
            .iter()
            .map(|angle| angle.angle_degrees().to_bits())
            .collect::<Vec<_>>(),
        vec![180.0_f64.to_bits(); 4]
    );
    assert!(
        fixture
            .geometry
            .solve_closed(
                &fixture.audit,
                fixture.fixed_face,
                &schedule.evaluate(1.0).unwrap(),
                1.0e-8,
            )
            .is_ok()
    );
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_rejects_a_wrong_three_carrier_word() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[3];
    let fixture = replace_hinges_v1(fixture, &[changed], |hinge| {
        let (start, end, axis) = carrier_v1(1, 9);
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            start,
            end,
            axis,
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_rejects_unsupported_carrier_label_sets() {
    let mut two_non_half = semidirect_fixture_v1(false, false, 0, false);
    let z_edge = two_non_half.cycle_edges[3];
    two_non_half = replace_hinges_v1(two_non_half, &[z_edge], |hinge| {
        let (start, end, axis) = carrier_v1(1, 9);
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            start,
            end,
            axis,
        )
    });
    for edge in [two_non_half.cycle_edges[1], two_non_half.cycle_edges[3]] {
        two_non_half = replace_profile_v1(two_non_half, edge, TestProfileV1::Collective);
    }

    let mut only_one_secondary_half_turn = semidirect_fixture_v1(false, false, 0, false);
    let y_edge = only_one_secondary_half_turn.cycle_edges[1];
    only_one_secondary_half_turn = replace_profile_v1(
        only_one_secondary_half_turn,
        y_edge,
        TestProfileV1::Collective,
    );

    let four_carriers = semidirect_fixture_v1(false, false, 0, false);
    let fourth_edge = four_carriers.cycle_edges[4];
    let four_carriers = replace_hinges_v1(four_carriers, &[fourth_edge], |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(0.0, 2.0, 0.0).unwrap(),
            Point3::new(1.0, 2.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        )
    });

    for fixture in [two_non_half, only_one_secondary_half_turn, four_carriers] {
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[derive(Clone, Copy)]
enum LineMutationV1 {
    Coincident,
    ParallelOffset,
    PairwiseSkewOnly,
    NonPerpendicularOneUlp,
}

fn mutate_y_carrier_v1(
    fixture: OrthogonalFixtureV1,
    mutation: LineMutationV1,
) -> OrthogonalFixtureV1 {
    let changed = fixture.cycle_edges[1];
    replace_hinges_v1(fixture, &[changed], |hinge| {
        let one_ulp = f64::from_bits(1);
        let (start, end, axis) = match mutation {
            LineMutationV1::Coincident => (
                Point3::new(2.0, 0.0, 0.0).unwrap(),
                Point3::new(3.0, 0.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ),
            LineMutationV1::ParallelOffset => (
                Point3::new(2.0, 1.0, 0.0).unwrap(),
                Point3::new(3.0, 1.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ),
            // X meets Z at the origin and this Y meets Z at z=1. Only the
            // exact X/Y incidence is broken, so pairwise checks cannot be
            // replaced by a single reference-pair check.
            LineMutationV1::PairwiseSkewOnly => (
                Point3::new(0.0, 0.0, 1.0).unwrap(),
                Point3::new(0.0, 1.0, 1.0).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
            ),
            LineMutationV1::NonPerpendicularOneUlp => (
                Point3::new(0.0, 0.0, 0.0).unwrap(),
                Point3::new(one_ulp, 1.0, 0.0).unwrap(),
                Point3::new(one_ulp, 1.0, 0.0).unwrap(),
            ),
        };
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
}

#[test]
fn orthogonal_half_turn_requires_every_exact_common_center_relation() {
    for mutation in [
        LineMutationV1::Coincident,
        LineMutationV1::ParallelOffset,
        LineMutationV1::PairwiseSkewOnly,
        LineMutationV1::NonPerpendicularOneUlp,
    ] {
        let fixture = mutate_y_carrier_v1(semidirect_fixture_v1(false, false, 0, false), mutation);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn orthogonal_half_turn_rejects_one_ulp_below_180_degrees() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[1];
    let fixture = replace_profile_v1(
        fixture,
        changed,
        TestProfileV1::Constant(180.0_f64.to_bits() - 1),
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_rejects_three_sample_profile_tamper() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[0];
    let normal = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let tampered = polynomial_schedule_v1(&fixture, ScheduleMutationV1::ThreeSampleMatch(changed));
    assert!(tampered.collective_profile_edges_v1().is_none());
    for parameter in [0.0, 0.5, 1.0] {
        let bits = |schedule: &CanonicalCycleScheduleV1| {
            schedule
                .evaluate(parameter)
                .unwrap()
                .as_slice()
                .iter()
                .find(|angle| angle.edge() == changed)
                .unwrap()
                .angle_degrees()
                .to_bits()
        };
        assert_eq!(bits(&normal), bits(&tampered));
    }
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &tampered,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_requires_complete_observer_partition_across_bridges() {
    let fixture = semidirect_fixture_v1(false, false, 0, true);
    assert_eq!(fixture.bridge_edges.len(), 1);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn orthogonal_half_turn_rejects_binding_and_tolerance_tamper() {
    let fixture = semidirect_fixture_v1(false, false, 0, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    for tolerance in [-f64::from_bits(1), f64::NAN] {
        assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            tolerance,
        ));
    }
    let other = semidirect_fixture_v1(false, false, 0, false);
    assert!(!block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));
}
