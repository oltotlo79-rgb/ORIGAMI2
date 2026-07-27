use ori_domain::FaceId;
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
        block_cut_orthogonal_half_turn::block_cut_orthogonal_half_turn_cycle_closure_premises_v1,
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

#[test]
fn finite_half_turn_group_strictly_certifies_exact_d3_cycle() {
    let fixture = d3_fixture_v1(false, false, 0, true);
    let schedule = polynomial_schedule_v1(&fixture);
    assert_eq!(fixture.geometry.face_ids().len(), 7);
    assert_eq!(fixture.geometry.hinges().len(), 7);
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    assert!(block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    let (prepared, _, _) = preparation::prepare_finite_half_turn_blocks_v1(&decomposition).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].group.order, 6);
    assert_eq!(prepared[0].group.carrier_count, 2);

    type ExistingIssuerV1 = fn(
        &MaterialHingeGraphGeometry,
        &MaterialHingeGraphAudit,
        FaceId,
        &CanonicalCycleScheduleV1,
        f64,
    ) -> bool;
    let existing_issuers: [(&str, ExistingIssuerV1); 15] = [
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
            "block_cut_orthogonal_half_turn",
            block_cut_orthogonal_half_turn_cycle_closure_premises_v1,
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
            "existing issuer {name} unexpectedly accepted the strict D3 fixture",
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
        .expect("the exact finite-half-turn issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn finite_half_turn_group_is_storage_root_and_orientation_invariant() {
    for fixed_face in 0..6 {
        for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
            let fixture = d3_fixture_v1(reverse_every_other, reverse_storage, fixed_face, true);
            let schedule = polynomial_schedule_v1(&fixture);
            assert!(block_cut_finite_half_turn_group_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            ));
            for parameter in [0.0, 0.5, 1.0] {
                assert!(
                    fixture
                        .geometry
                        .solve_closed(
                            &fixture.audit,
                            fixture.fixed_face,
                            &schedule.evaluate(parameter).unwrap(),
                            1.0e-8,
                        )
                        .is_ok()
                );
            }
        }
    }
}

#[test]
fn finite_half_turn_group_ignores_axis_and_assignment_sign_at_180() {
    let fixture = d3_fixture_v1(false, false, 0, true);
    let changed = fixture.cycle_edges[1];
    let fixture = replace_hinges_v1(fixture, &[changed], |hinge| {
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
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn finite_half_turn_group_contracts_zero_and_leaves_bridges_relation_free() {
    for profile in [
        TestProfileV1::Constant(0.0_f64.to_bits()),
        TestProfileV1::OtherNonconstant,
    ] {
        let fixture = d3_fixture_v1(false, false, 0, true);
        let bridge = fixture.bridge_edges[0];
        let fixture = replace_profile_v1(fixture, bridge, profile);
        let schedule = polynomial_schedule_v1(&fixture);
        assert!(block_cut_finite_half_turn_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn finite_half_turn_group_rejects_an_odd_single_generator_cycle() {
    let fixture = odd_half_turn_fixture_v1();
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[derive(Debug, Clone, Copy)]
enum InfiniteMutationV1 {
    DirectionOneUlp,
    ParallelOffset,
    SkewOffset,
}

fn mutate_second_carrier_v1(
    fixture: FiniteHalfTurnFixtureV1,
    mutation: InfiniteMutationV1,
) -> FiniteHalfTurnFixtureV1 {
    let selected = [
        fixture.cycle_edges[1],
        fixture.cycle_edges[3],
        fixture.cycle_edges[5],
    ];
    replace_hinges_v1(fixture, &selected, |hinge| {
        let (raw, center) = match mutation {
            InfiniteMutationV1::DirectionOneUlp => (
                [f64::from_bits(1.0_f64.to_bits() + 1), 0.0, 1.0],
                [3.0, 5.0, 7.0],
            ),
            InfiniteMutationV1::ParallelOffset => ([1.0, 1.0, 0.0], [3.0, 5.0, 8.0]),
            InfiniteMutationV1::SkewOffset => ([1.0, 0.0, 1.0], [3.0, 6.0, 7.0]),
        };
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(center[0], center[1], center[2]).unwrap(),
            Point3::new(center[0] + raw[0], center[1] + raw[1], center[2] + raw[2]).unwrap(),
            normalized_v1(raw),
        )
    })
}

#[test]
fn finite_half_turn_group_rejects_nonfinite_affine_subgroups() {
    for mutation in [
        InfiniteMutationV1::DirectionOneUlp,
        InfiniteMutationV1::ParallelOffset,
        InfiniteMutationV1::SkewOffset,
    ] {
        let fixture = mutate_second_carrier_v1(d3_fixture_v1(false, false, 0, false), mutation);
        let schedule = polynomial_schedule_v1(&fixture);
        assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn finite_half_turn_group_rejects_one_ulp_below_180() {
    let fixture = d3_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[0];
    let fixture = replace_profile_v1(
        fixture,
        changed,
        TestProfileV1::Constant(180.0_f64.to_bits() - 1),
    );
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn finite_half_turn_group_rejects_nonconstant_three_sample_half_turn() {
    let fixture = d3_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[0];
    let fixture = replace_profile_v1(fixture, changed, TestProfileV1::SampledHalfTurn);
    let schedule = polynomial_schedule_v1(&fixture);
    for parameter in [0.0, 0.5, 1.0] {
        let angle = schedule
            .evaluate(parameter)
            .unwrap()
            .as_slice()
            .iter()
            .find(|angle| angle.edge() == changed)
            .unwrap()
            .angle_degrees();
        assert_eq!(angle.to_bits(), 180.0_f64.to_bits());
    }
    assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn finite_half_turn_group_rejects_binding_and_tolerance_tamper() {
    let fixture = d3_fixture_v1(false, false, 0, true);
    let schedule = polynomial_schedule_v1(&fixture);
    for tolerance in [-f64::from_bits(1), f64::NAN] {
        assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            tolerance,
        ));
    }
    let other = d3_fixture_v1(false, false, 0, true);
    assert!(!block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));
}
