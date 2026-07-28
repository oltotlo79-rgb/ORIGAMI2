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
        block_cut_finite_half_turn_group::block_cut_finite_half_turn_group_cycle_closure_premises_v1,
        block_cut_free_word::block_cut_free_word_cycle_closure_premises_v1,
        block_cut_generalized_dihedral::block_cut_generalized_dihedral_cycle_closure_premises_v1,
        block_cut_orthogonal_half_turn::block_cut_orthogonal_half_turn_cycle_closure_premises_v1,
        bridge_motion::bridge_only_motion_cycle_closure_premises_v1,
        coaxial_profile_lattice::coaxial_profile_lattice_cycle_closure_premises_v1,
        collective_flat_stack_cycle_closure_premises_v1,
        dense_grid::dense_parallel_grid_cycle_closure_premises_v1,
        even_single_vertex_opposite_pair_cycle_closure_premises_v1,
        exact_cut_carrier::exact_cut_carrier_cycle_closure_premises_v1,
        exact_generator_word::{
            exact_generator_line_v1, exact_generator_word_cycle_closure_premises_v1,
        },
        orthogonal_inverse_pair_cycle_closure_premises_v1,
        symmetric_rational_kawasaki_cycle_closure_premises_v1,
        theta_opposite_pair_cycle_closure_premises_v1,
    },
};

#[test]
fn cardinal_rotation_group_strictly_certifies_octahedral_relation() {
    let fixture = octahedral_fixture_v1(false, false, 0, true);
    let schedule = polynomial_schedule_v1(&fixture);
    assert_eq!(fixture.geometry.face_ids().len(), 7);
    assert_eq!(fixture.geometry.hinges().len(), 7);
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    assert_ne!(
        schedule
            .derivative_bound(fixture.bridge_edges[0])
            .unwrap()
            .to_bits(),
        0.0_f64.to_bits()
    );
    assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    let (prepared, bounds, classifications, exact_relations) =
        preparation::prepare_cardinal_rotation_blocks_v1(&decomposition).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].carrier_count, 2);
    assert_eq!(bounds.exact_relations, 1);
    assert_eq!(classifications, bounds.key_classifications);
    assert_eq!(exact_relations, bounds.exact_relations);

    type ExistingIssuerV1 = fn(
        &MaterialHingeGraphGeometry,
        &MaterialHingeGraphAudit,
        FaceId,
        &CanonicalCycleScheduleV1,
        f64,
    ) -> bool;
    let existing_issuers: [(&str, ExistingIssuerV1); 16] = [
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
            "block_cut_finite_half_turn_group",
            block_cut_finite_half_turn_group_cycle_closure_premises_v1,
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
            "existing issuer {name} unexpectedly accepted the strict octahedral fixture",
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
        .expect("the exact cardinal issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn cardinal_rotation_group_is_storage_root_and_orientation_invariant() {
    for fixed_face in 0..6 {
        for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
            let fixture =
                octahedral_fixture_v1(reverse_every_other, reverse_storage, fixed_face, true);
            let schedule = polynomial_schedule_v1(&fixture);
            assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
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

    let mut fixture = octahedral_fixture_v1(false, true, 0, true);
    let bridge = fixture.bridge_edges[0];
    let hinge = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == bridge)
        .unwrap();
    let leaf_face = [hinge.left_face(), hinge.right_face()]
        .into_iter()
        .find(|face| !fixture.cycle_faces.contains(face))
        .expect("the moving bridge must have one leaf face");
    fixture.fixed_face = leaf_face;
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(
        fixture
            .geometry
            .solve_closed(
                &fixture.audit,
                fixture.fixed_face,
                &schedule.evaluate(0.5).unwrap(),
                1.0e-8,
            )
            .is_ok()
    );
}

#[test]
fn cardinal_rotation_group_certifies_three_axis_conjugation() {
    for fixed_face in 0..4 {
        for (reverse_every_other, reverse_storage) in [(false, false), (false, true), (true, false)]
        {
            let fixture =
                three_axis_conjugation_fixture_v1(reverse_every_other, reverse_storage, fixed_face);
            let schedule = polynomial_schedule_v1(&fixture);
            assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            ));
            let decomposition =
                prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule)
                    .unwrap();
            let (prepared, bounds, _, _) =
                preparation::prepare_cardinal_rotation_blocks_v1(&decomposition).unwrap();
            assert_eq!(prepared[0].carrier_count, 3);
            assert_eq!(bounds.exact_relations, 3);
        }
    }
}

#[test]
fn cardinal_rotation_group_is_world_frame_invariant() {
    let fixture = octahedral_fixture_v1(false, false, 0, false);
    let first_carrier = [
        fixture.cycle_edges[0],
        fixture.cycle_edges[2],
        fixture.cycle_edges[4],
    ]
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    let selected = fixture.cycle_edges.clone();
    let fixture = replace_hinges_v1(fixture, &selected, |hinge| {
        let raw = if first_carrier.contains(&hinge.edge()) {
            [1.0, 1.0, 0.0]
        } else {
            [1.0, -1.0, 0.0]
        };
        let center = [3.0, 5.0, 7.0];
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(center[0], center[1], center[2]).unwrap(),
            Point3::new(center[0] + raw[0], center[1] + raw[1], center[2] + raw[2]).unwrap(),
            normalized_v1(raw),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
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
                .is_ok(),
            "the exact world-frame theorem must agree with the native solver at u={parameter}"
        );
    }
}

#[test]
fn cardinal_rotation_group_composes_quarter_and_half_turns_exactly() {
    let word = [
        (0, FoldAssignment::Mountain),
        (0, FoldAssignment::Mountain),
        (0, FoldAssignment::Mountain),
    ];
    let fixture = word_fixture_v1(&word, false, false, 0, false);
    let half_turn = fixture.cycle_edges[2];
    let fixture = replace_profile_v1(
        fixture,
        half_turn,
        TestProfileV1::Constant(180.0_f64.to_bits()),
    );
    let fixture = replace_hinges_v1(fixture, &[half_turn], |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            FoldAssignment::Valley,
            hinge.left_face(),
            hinge.right_face(),
            hinge.end(),
            hinge.start(),
            Point3::new(-hinge.axis().x(), -hinge.axis().y(), -hinge.axis().z()).unwrap(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn cardinal_rotation_group_contracts_zero_and_leaves_bridges_relation_free() {
    for profile in [
        TestProfileV1::Constant(0.0_f64.to_bits()),
        TestProfileV1::OtherNonconstant,
    ] {
        let fixture = octahedral_fixture_v1(false, false, 0, true);
        let bridge = fixture.bridge_edges[0];
        let fixture = replace_profile_v1(fixture, bridge, profile);
        let schedule = polynomial_schedule_v1(&fixture);
        assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn cardinal_rotation_group_rejects_a_wrong_cardinal_word() {
    let word = [
        (0, FoldAssignment::Mountain),
        (1, FoldAssignment::Mountain),
        (0, FoldAssignment::Mountain),
        (1, FoldAssignment::Mountain),
    ];
    let fixture = word_fixture_v1(&word, false, false, 0, false);
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(
        !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        )
    );
}

#[derive(Debug, Clone, Copy)]
enum CarrierMutationV1 {
    DirectionOneUlp,
    ParallelOffset,
    SkewOffset,
}

fn mutate_second_carrier_v1(
    fixture: CardinalRotationFixtureV1,
    mutation: CarrierMutationV1,
) -> CardinalRotationFixtureV1 {
    let selected = [
        fixture.cycle_edges[1],
        fixture.cycle_edges[3],
        fixture.cycle_edges[5],
    ];
    replace_hinges_v1(fixture, &selected, |hinge| {
        let (raw, center) = match mutation {
            CarrierMutationV1::DirectionOneUlp => {
                let one_ulp_at_center = f64::from_bits(3.0_f64.to_bits() + 1) - 3.0_f64;
                ([one_ulp_at_center, 1.0, 0.0], [3.0, 5.0, 7.0])
            }
            CarrierMutationV1::ParallelOffset => ([1.0, 0.0, 0.0], [3.0, 5.0, 8.0]),
            CarrierMutationV1::SkewOffset => ([0.0, 1.0, 0.0], [3.0, 5.0, 8.0]),
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
fn cardinal_rotation_group_rejects_nonorthogonal_or_nonconcurrent_carriers() {
    for mutation in [
        CarrierMutationV1::DirectionOneUlp,
        CarrierMutationV1::ParallelOffset,
        CarrierMutationV1::SkewOffset,
    ] {
        let fixture =
            mutate_second_carrier_v1(octahedral_fixture_v1(false, false, 0, false), mutation);
        assert!(
            fixture
                .geometry
                .hinges()
                .iter()
                .all(|hinge| exact_generator_line_v1(hinge).is_some()),
            "the carrier mutation must remain native before its exact relation is rejected",
        );
        let schedule = polynomial_schedule_v1(&fixture);
        assert!(
            !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            )
        );
    }
}

#[test]
fn cardinal_rotation_group_rejects_adjacent_cardinal_angles() {
    for bits in [
        90.0_f64.to_bits() - 1,
        90.0_f64.to_bits() + 1,
        180.0_f64.to_bits() - 1,
    ] {
        let fixture = octahedral_fixture_v1(false, false, 0, false);
        let changed = fixture.cycle_edges[0];
        let fixture = replace_profile_v1(fixture, changed, TestProfileV1::Constant(bits));
        let schedule = polynomial_schedule_v1(&fixture);
        assert!(
            !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                0.0,
            )
        );
    }
}

#[test]
fn cardinal_rotation_group_rejects_nonconstant_three_sample_quarter_turn() {
    let fixture = octahedral_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[0];
    let fixture = replace_profile_v1(fixture, changed, TestProfileV1::SampledQuarterTurn);
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
        assert_eq!(angle.to_bits(), 90.0_f64.to_bits());
    }
    assert!(
        !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        )
    );
}

#[test]
fn cardinal_rotation_group_preserves_quarter_turn_sign() {
    let fixture = octahedral_fixture_v1(false, false, 0, false);
    let changed = fixture.cycle_edges[0];
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
    assert!(
        !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        )
    );
}

#[test]
fn cardinal_rotation_group_rejects_a_one_ulp_third_carrier_offset() {
    let fixture = three_axis_conjugation_fixture_v1(false, false, 0);
    let third = fixture.cycle_edges[3];
    let shifted_x = f64::from_bits(3.0_f64.to_bits() + 1);
    let fixture = replace_hinges_v1(fixture, &[third], |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(shifted_x, 5.0, 7.0).unwrap(),
            Point3::new(shifted_x, 5.0, 8.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(
        !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        )
    );
}

#[test]
fn cardinal_rotation_group_rejects_binding_and_tolerance_tamper() {
    let fixture = octahedral_fixture_v1(false, false, 0, true);
    let schedule = polynomial_schedule_v1(&fixture);
    for tolerance in [-f64::from_bits(1), f64::NAN] {
        assert!(
            !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                tolerance,
            )
        );
    }
    let other = octahedral_fixture_v1(false, false, 0, true);
    assert!(
        !block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
            &other.geometry,
            &other.audit,
            other.fixed_face,
            &schedule,
            0.0,
        )
    );
}
