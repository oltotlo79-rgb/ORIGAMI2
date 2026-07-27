use ori_domain::EdgeId;
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3, TreeHinge,
    graph::{
        block_cut_carrier_free_product::block_cut_carrier_free_product_cycle_closure_premises_v1,
        block_cut_coaxial::block_cut_coaxial_cycle_closure_premises_v1,
        block_cut_free_word::block_cut_free_word_cycle_closure_premises_v1,
        bridge_motion::bridge_only_motion_cycle_closure_premises_v1,
        coaxial_profile_lattice::coaxial_profile_lattice_cycle_closure_premises_v1,
        exact_cut_carrier::exact_cut_carrier_cycle_closure_premises_v1,
        exact_generator_word::exact_generator_word_cycle_closure_premises_v1,
        orthogonal_inverse_pair_cycle_closure_premises_v1,
    },
};

fn angle_bits_v1(schedule: &CanonicalCycleScheduleV1, edge: EdgeId, parameter: f64) -> u64 {
    schedule
        .evaluate(parameter)
        .unwrap()
        .as_slice()
        .iter()
        .find(|angle| angle.edge() == edge)
        .unwrap()
        .angle_degrees()
        .to_bits()
}

#[test]
fn generalized_dihedral_strictly_certifies_half_turn_conjugation() {
    let fixture = dihedral_fixture_v1(false, false, false, false);
    assert_eq!(fixture.geometry.face_ids().len(), 4);
    assert_eq!(fixture.geometry.hinges().len(), 4);
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert_eq!(schedule.collective_profile_edges_v1().unwrap().len(), 2);
    assert!(block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!bridge_only_motion_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(!orthogonal_inverse_pair_cycle_closure_premises_v1(
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
        .expect("the exact generalized-dihedral issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn generalized_dihedral_is_invariant_to_storage_and_hinge_orientation() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = dihedral_fixture_v1(reverse_every_other, reverse_storage, false, false);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_generalized_dihedral_cycle_closure_premises_v1(
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
                    .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-8)
                    .is_ok()
            );
        }
    }
}

#[test]
fn generalized_dihedral_ignores_only_the_exact_half_turn_storage_sign() {
    let fixture = dihedral_fixture_v1(false, false, false, false);
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
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn generalized_dihedral_uses_a_canonical_factor_when_both_are_half_turns() {
    for (reverse_every_other, reverse_storage) in [(false, false), (true, true)] {
        let mut fixture = dihedral_fixture_v1(reverse_every_other, reverse_storage, false, false);
        for edge in [fixture.cycle_edges[1], fixture.cycle_edges[3]] {
            fixture =
                replace_profile_v1(fixture, edge, TestProfileV1::Constant(180.0_f64.to_bits()));
        }
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_generalized_dihedral_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn generalized_dihedral_rejects_the_wrong_primary_inverse_word() {
    let fixture = dihedral_fixture_v1(false, false, true, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[derive(Clone, Copy)]
enum LineMutationV1 {
    Coincident,
    ParallelOffset,
    PerpendicularSkew,
    NonPerpendicularOneUlp,
}

fn mutate_half_turn_lines_v1(
    fixture: DihedralFixtureV1,
    mutation: LineMutationV1,
) -> DihedralFixtureV1 {
    let half_turn_edges = [fixture.cycle_edges[0], fixture.cycle_edges[2]];
    replace_hinges_v1(fixture, &half_turn_edges, |hinge| {
        let along = hinge.start().y();
        let one_ulp = f64::from_bits(1);
        let (start, end, axis) = match mutation {
            LineMutationV1::Coincident => (
                Point3::new(along, 0.0, 0.0).unwrap(),
                Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ),
            LineMutationV1::ParallelOffset => (
                Point3::new(along, 1.0, 0.0).unwrap(),
                Point3::new(along + 1.0, 1.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ),
            LineMutationV1::PerpendicularSkew => (
                Point3::new(0.0, along, one_ulp).unwrap(),
                Point3::new(0.0, along + 1.0, one_ulp).unwrap(),
                Point3::new(0.0, 1.0, 0.0).unwrap(),
            ),
            LineMutationV1::NonPerpendicularOneUlp => (
                Point3::new(along * one_ulp, along, 0.0).unwrap(),
                Point3::new((along + 1.0) * one_ulp, along + 1.0, 0.0).unwrap(),
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
fn generalized_dihedral_rejects_nonunique_or_nonperpendicular_line_relations() {
    for mutation in [
        LineMutationV1::Coincident,
        LineMutationV1::ParallelOffset,
        LineMutationV1::PerpendicularSkew,
        LineMutationV1::NonPerpendicularOneUlp,
    ] {
        let fixture =
            mutate_half_turn_lines_v1(dihedral_fixture_v1(false, false, false, false), mutation);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn generalized_dihedral_rejects_a_half_turn_one_ulp_below_180_degrees() {
    let fixture = dihedral_fixture_v1(false, false, false, false);
    let changed = fixture.cycle_edges[0];
    let fixture = replace_profile_v1(
        fixture,
        changed,
        TestProfileV1::Constant(180.0_f64.to_bits() - 1),
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn generalized_dihedral_rejects_three_sample_matched_profile_tamper() {
    let fixture = dihedral_fixture_v1(false, false, false, false);
    let changed = fixture.cycle_edges[1];
    let normal = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let tampered = polynomial_schedule_v1(&fixture, ScheduleMutationV1::ThreeSampleMatch(changed));
    assert!(tampered.collective_profile_edges_v1().is_none());
    for parameter in [0.0, 0.5, 1.0] {
        assert_eq!(
            angle_bits_v1(&normal, changed, parameter),
            angle_bits_v1(&tampered, changed, parameter)
        );
    }
    assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &tampered,
        0.0,
    ));
}

#[test]
fn generalized_dihedral_requires_observer_partition_across_bridges() {
    let fixture = dihedral_fixture_v1(false, false, false, true);
    assert_eq!(fixture.bridge_edges.len(), 1);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn generalized_dihedral_rejects_binding_and_tolerance_tamper() {
    let fixture = dihedral_fixture_v1(false, false, false, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    for tolerance in [-f64::from_bits(1), f64::NAN] {
        assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            tolerance,
        ));
    }
    let other = dihedral_fixture_v1(false, false, false, false);
    assert!(!block_cut_generalized_dihedral_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));
}
