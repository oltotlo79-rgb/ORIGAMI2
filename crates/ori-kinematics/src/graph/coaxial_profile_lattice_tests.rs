use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3, TreeHinge,
    graph::exact_generator_word::exact_generator_word_cycle_closure_premises_v1,
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
fn coaxial_lattice_certifies_multi_profile_cube_commutators() {
    let fixture = coaxial_cube_fixture_v1(false, false);
    assert_eq!(fixture.geometry.face_ids().len(), 16);
    assert_eq!(fixture.geometry.hinges().len(), 56);
    assert_eq!(fixture.audit.closure_hinges().len(), 41);
    assert_eq!(fixture.zero_edges.len(), 8);
    assert_eq!(fixture.groups.len(), 8);

    let first = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving_edges[0])
        .unwrap();
    let second = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving_edges[1])
        .unwrap();
    assert_ne!(first.start(), second.start());
    let (first_line, _) = exact_generator_line_v1(first).unwrap();
    let (second_line, _) = exact_generator_line_v1(second).unwrap();
    assert_eq!(first_line, second_line);

    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(coaxial_profile_lattice_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }

    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(
        !exact_generator_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ),
        "the free-group issuer must not silently assume coaxial commutativity"
    );
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
        .expect("the exact coaxial issuer must bypass interval subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn coaxial_lattice_is_invariant_to_storage_order_and_hinge_reversal() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = coaxial_cube_fixture_v1(reverse_every_other, reverse_storage);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(coaxial_profile_lattice_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));

        // These numerical solves are regression observations only. The exact
        // lattice proof above does not use samples or a floating tolerance.
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
fn coaxial_lattice_rejects_parallel_offsets_and_noncoaxial_axes_exactly() {
    let fixture = coaxial_cube_fixture_v1(false, false);
    let changed = fixture.moving_edges[0];
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
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let fixture = coaxial_cube_fixture_v1(false, false);
    let changed = fixture.constant_angles[0].0;
    let fixture = replace_hinge_v1(fixture, changed, |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(0.0, 0.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn coaxial_lattice_rejects_a_second_or_sample_matched_nonconstant_profile() {
    let fixture = coaxial_cube_fixture_v1(false, false);
    let changed = fixture.moving_edges[0];
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::SecondNonconstant(changed));
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let normal = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let sample_matched =
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::ThreeSampleMatch(changed));
    for parameter in [0.0, 0.5, 1.0] {
        assert_eq!(
            angle_bits_v1(&normal, changed, parameter),
            angle_bits_v1(&sample_matched, changed, parameter)
        );
    }
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &sample_matched,
        0.0,
    ));

    let changed = fixture.constant_angles[0].0;
    let one_ulp_constant = polynomial_schedule_v1(
        &fixture,
        ScheduleMutationV1::ConstantAngle(changed, 30.0_f64.to_bits() + 1),
    );
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &one_ulp_constant,
        0.0,
    ));
}

#[test]
fn constant_profile_keys_preserve_conservative_binary64_semantics() {
    assert_eq!(exact_constant_profile_v1(0.0_f64.to_bits()), Some(None));
    assert_eq!(exact_constant_profile_v1((-0.0_f64).to_bits()), Some(None));
    assert_eq!(exact_constant_profile_v1((-1.0_f64).to_bits()), None);
    assert_eq!(exact_constant_profile_v1(f64::INFINITY.to_bits()), None);
    assert_eq!(exact_constant_profile_v1(f64::NAN.to_bits()), None);

    let minimum_subnormal = exact_constant_profile_v1(1).unwrap().unwrap();
    let ninety = exact_constant_profile_v1(90.0_f64.to_bits())
        .unwrap()
        .unwrap();
    let one_eighty = exact_constant_profile_v1(180.0_f64.to_bits())
        .unwrap()
        .unwrap();
    let below_one_eighty = exact_constant_profile_v1(180.0_f64.to_bits() - 1)
        .unwrap()
        .unwrap();
    assert_ne!(minimum_subnormal, ninety);
    assert_ne!(ninety, one_eighty);
    assert_ne!(below_one_eighty, one_eighty);
    assert_eq!(exact_constant_profile_v1(180.0_f64.to_bits() + 1), None);
}

#[test]
fn exact_carrier_sign_tracks_assignment_and_storage_direction() {
    let namespace = ProjectId::new();
    let left = FaceId::derive_v5(namespace, b"coaxial-sign-left");
    let right = FaceId::derive_v5(namespace, b"coaxial-sign-right");
    let edge = EdgeId::derive_v5(namespace, b"coaxial-sign-edge");
    let start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let end = Point3::new(1.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let mountain = TreeHinge::new_for_test(
        edge,
        FoldAssignment::Mountain,
        left,
        right,
        start,
        end,
        axis,
    );
    let valley =
        TreeHinge::new_for_test(edge, FoldAssignment::Valley, left, right, start, end, axis);
    let reversed = TreeHinge::new_for_test(
        edge,
        FoldAssignment::Mountain,
        right,
        left,
        end,
        start,
        Point3::new(-1.0, 0.0, 0.0).unwrap(),
    );
    let (mountain_line, mountain_sign) = exact_generator_line_v1(&mountain).unwrap();
    let (valley_line, valley_sign) = exact_generator_line_v1(&valley).unwrap();
    let (reversed_line, reversed_sign) = exact_generator_line_v1(&reversed).unwrap();
    assert_eq!(mountain_line, valley_line);
    assert_eq!(mountain_line, reversed_line);
    assert_eq!(mountain_sign, 1);
    assert_eq!(valley_sign, -1);
    assert_eq!(reversed_sign, -1);
}

#[test]
fn coaxial_lattice_rejects_assignment_tamper_binding_mismatch_and_bad_tolerance() {
    let fixture = coaxial_cube_fixture_v1(false, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        -f64::from_bits(1),
    ));
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        f64::NAN,
    ));

    let other = coaxial_cube_fixture_v1(false, false);
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));

    let changed = fixture.moving_edges[0];
    let fixture = replace_hinge_v1(fixture, changed, |hinge| {
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
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}
