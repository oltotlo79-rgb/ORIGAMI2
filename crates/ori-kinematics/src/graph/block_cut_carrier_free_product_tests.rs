use ori_domain::EdgeId;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3, TreeHinge,
    graph::{
        block_cut_coaxial::block_cut_coaxial_cycle_closure_premises_v1,
        block_cut_free_word::block_cut_free_word_cycle_closure_premises_v1,
        coaxial_profile_lattice::coaxial_profile_lattice_cycle_closure_premises_v1,
        exact_generator_word::exact_generator_word_cycle_closure_premises_v1,
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
fn carrier_free_product_strictly_combines_coaxial_commutation_and_carrier_words() {
    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        false,
        false,
        false,
    );
    assert_eq!(fixture.geometry.face_ids().len(), 6);
    assert_eq!(fixture.geometry.hinges().len(), 6);
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    assert_eq!(fixture.cycle_edges.len(), 6);

    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(schedule.collective_profile_edges_v1().is_some());
        assert!(block_cut_carrier_free_product_cycle_closure_premises_v1(
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
        assert!(!block_cut_coaxial_cycle_closure_premises_v1(
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
    }

    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
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
        .expect("the exact carrier free-product issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn carrier_free_product_is_invariant_to_storage_and_hinge_orientation() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = word_fixture_v1(
            TestWordV1::SameCarrierCommutatorThenInverse,
            true,
            false,
            reverse_every_other,
            reverse_storage,
        );
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_carrier_free_product_cycle_closure_premises_v1(
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
fn carrier_free_product_rejects_a_different_carrier_commutator() {
    let fixture = word_fixture_v1(
        TestWordV1::DifferentCarrierCommutator,
        true,
        false,
        false,
        false,
    );
    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn carrier_free_product_rejects_an_exactly_offset_parallel_carrier() {
    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        false,
        false,
        false,
        false,
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let changed = fixture.cycle_edges[2];
    let subnormal = f64::from_bits(1);
    let fixture = replace_hinge_v1(fixture, changed, |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(hinge.start().x(), subnormal, hinge.start().z()).unwrap(),
            Point3::new(hinge.end().x(), subnormal, hinge.end().z()).unwrap(),
            hinge.axis(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn carrier_free_product_rejects_three_sample_matched_profile_tamper() {
    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        false,
        false,
        false,
    );
    let changed = fixture.cycle_edges[0];
    let normal = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let tampered = polynomial_schedule_v1(&fixture, ScheduleMutationV1::ThreeSampleMatch(changed));
    assert!(tampered.collective_profile_edges_v1().is_none());
    for parameter in [0.0, 0.5, 1.0] {
        assert_eq!(
            angle_bits_v1(&normal, changed, parameter),
            angle_bits_v1(&tampered, changed, parameter)
        );
    }
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &tampered,
        0.0,
    ));
}

#[test]
fn carrier_free_product_requires_observer_partition_across_active_bridges() {
    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        true,
        false,
        false,
    );
    assert_eq!(fixture.bridge_edges.len(), 1);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        false,
        false,
        false,
    );
    let changed = fixture.cycle_edges[0];
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::SecondNonconstant(changed));
    assert!(schedule.collective_profile_edges_v1().is_none());
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn carrier_free_product_rejects_binding_and_tolerance_tamper() {
    let fixture = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        false,
        false,
        false,
    );
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    for tolerance in [-f64::from_bits(1), f64::NAN] {
        assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            tolerance,
        ));
    }
    let other = word_fixture_v1(
        TestWordV1::SameCarrierCommutatorThenInverse,
        true,
        false,
        false,
        false,
    );
    assert!(!block_cut_carrier_free_product_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));
}
