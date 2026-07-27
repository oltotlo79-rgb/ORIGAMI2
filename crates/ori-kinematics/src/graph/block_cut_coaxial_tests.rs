use ori_domain::EdgeId;
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, Point3, TreeHinge,
    graph::{
        block_cut_decomposition::prepare_contracted_block_cut_v1,
        coaxial_profile_lattice::coaxial_profile_lattice_cycle_closure_premises_v1,
        exact_cut_carrier::exact_cut_carrier_cycle_closure_premises_v1,
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
fn block_cut_certifies_distinct_carrier_commutator_blocks_without_cross_commuting() {
    let fixture = two_block_fixture_v1(false, false, false, false);
    assert_eq!(fixture.geometry.face_ids().len(), 8);
    assert_eq!(fixture.geometry.hinges().len(), 9);
    assert_eq!(fixture.audit.closure_hinges().len(), 2);
    assert_eq!(fixture.x_block_edges.len(), 4);
    assert_eq!(fixture.y_block_edges.len(), 4);
    assert_eq!(fixture.bridge_edges.len(), 1);

    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(block_cut_coaxial_cycle_closure_premises_v1(
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
        assert!(!exact_generator_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }

    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    let mut shapes = decomposition
        .blocks()
        .iter()
        .map(|block| (block.edge_indices().len(), block.vertices().len()))
        .collect::<Vec<_>>();
    shapes.sort_unstable();
    assert_eq!(shapes, vec![(1, 2), (4, 4), (4, 4)]);

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
        .expect("the exact block-cut issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn block_cut_is_invariant_to_storage_and_hinge_orientation() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = two_block_fixture_v1(reverse_every_other, reverse_storage, false, false);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_coaxial_cycle_closure_premises_v1(
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
fn bridge_blocks_do_not_read_carrier_or_profile_equality() {
    let fixture = two_block_fixture_v1(false, false, true, true);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert_eq!(fixture.bridge_edges.len(), 2);
    assert!(
        schedule.collective_profile_edges_v1().is_none(),
        "the two bridge-only nonconstant profiles are intentionally distinct"
    );
    assert!(block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn block_cut_contains_the_exact_cut_parallel_sign_case_after_zero_contraction() {
    let fixture = exact_cut_fixture_v1(true);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert_eq!(fixture.zero_edges.len(), 2);
    assert!(exact_cut_carrier_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    assert!(block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    assert_eq!(decomposition.blocks().len(), 1);
    assert_eq!(decomposition.blocks()[0].edge_indices().len(), 2);
    assert_eq!(decomposition.blocks()[0].vertices().len(), 2);

    let fixture = exact_cut_fixture_v1(false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!exact_cut_carrier_cycle_closure_premises_v1(
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
}

#[test]
fn block_cut_rejects_a_noncoaxial_edge_inside_only_its_cyclic_block() {
    let fixture = two_block_fixture_v1(false, false, false, false);
    let changed = fixture.x_block_edges[0];
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
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn block_cut_rejects_second_and_three_sample_matched_cyclic_profiles() {
    let fixture = two_block_fixture_v1(false, false, false, false);
    let changed = fixture.x_block_edges[0];
    let second = polynomial_schedule_v1(&fixture, ScheduleMutationV1::SecondNonconstant(changed));
    assert!(second.collective_profile_edges_v1().is_none());
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &second,
        0.0,
    ));

    let normal = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let sample_matched =
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::ThreeSampleMatch(changed));
    assert!(sample_matched.collective_profile_edges_v1().is_none());
    for parameter in [0.0, 0.5, 1.0] {
        assert_eq!(
            angle_bits_v1(&normal, changed, parameter),
            angle_bits_v1(&sample_matched, changed, parameter)
        );
    }
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &sample_matched,
        0.0,
    ));
}

#[test]
fn block_cut_consumes_the_collective_observer_as_an_exact_partition() {
    let fixture = two_block_fixture_v1(false, false, false, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let mut observed = schedule.collective_profile_edges_v1().unwrap();
    observed.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut expected = fixture
        .profiles
        .iter()
        .filter_map(|(edge, profile)| {
            (*profile == TestScheduleProfileV1::Collective).then_some(*edge)
        })
        .collect::<Vec<_>>();
    expected.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert_eq!(observed, expected);

    for (edge, profile) in &fixture.profiles {
        let exact_constant = schedule.is_exact_constant_profile_v1(*edge);
        assert_eq!(
            exact_constant,
            matches!(profile, TestScheduleProfileV1::Constant(_))
        );
        assert_eq!(
            observed
                .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
                .is_ok(),
            !exact_constant
        );
    }
}

#[test]
fn block_cut_rejects_active_self_loops_created_by_exact_zero_contraction() {
    let fixture = contracted_self_loop_fixture_v1();
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert_eq!(fixture.zero_edges.len(), 2);
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn block_cut_rejects_sign_binding_and_tolerance_tamper() {
    let fixture = two_block_fixture_v1(false, false, false, false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        -f64::from_bits(1),
    ));
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        f64::NAN,
    ));

    let other = two_block_fixture_v1(false, false, false, false);
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));

    let changed = fixture.y_block_edges[0];
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
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}
