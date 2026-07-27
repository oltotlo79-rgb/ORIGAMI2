use ori_domain::EdgeId;

use super::test_support::*;
use super::*;
use crate::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    graph::{
        block_cut_coaxial::block_cut_coaxial_cycle_closure_premises_v1,
        block_cut_decomposition::prepare_contracted_block_cut_v1,
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
fn block_cut_free_word_certifies_noncoaxial_words_with_opaque_bridges() {
    let fixture = two_block_free_word_fixture_v1(false, false, false, false, true);
    assert_eq!(fixture.geometry.face_ids().len(), 9);
    assert_eq!(fixture.geometry.hinges().len(), 10);
    assert_eq!(fixture.audit.closure_hinges().len(), 2);
    assert_eq!(fixture.cyclic_blocks.len(), 2);
    assert!(fixture.cyclic_blocks.iter().all(|block| block.len() == 4));
    assert_eq!(fixture.bridge_edges.len(), 2);

    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(
            schedule.collective_profile_edges_v1().is_none(),
            "the two bridge-only moving profiles are intentionally distinct"
        );
        assert!(block_cut_free_word_cycle_closure_premises_v1(
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
        assert!(!block_cut_coaxial_cycle_closure_premises_v1(
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
        .map(|block| {
            (
                block.edge_indices().len(),
                block.vertices().len(),
                block.is_bridge(),
            )
        })
        .collect::<Vec<_>>();
    shapes.sort_unstable();
    assert_eq!(
        shapes,
        vec![(1, 2, true), (1, 2, true), (4, 4, false), (4, 4, false)]
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
        .expect("the exact block-cut free-word issuer must bypass subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn block_cut_free_word_is_invariant_to_storage_and_hinge_orientation() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = two_block_free_word_fixture_v1(
            reverse_every_other,
            reverse_storage,
            false,
            false,
            true,
        );
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(block_cut_free_word_cycle_closure_premises_v1(
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
fn block_cut_free_word_rejects_nonreduced_commutator_cycles() {
    let fixture = two_block_free_word_fixture_v1(false, false, true, false, false);
    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(!block_cut_free_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn block_cut_free_word_accepts_exact_collective_partition_for_both_schedule_kinds() {
    let fixture = two_block_free_word_fixture_v1(false, false, false, true, false);
    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        let mut observed = schedule.collective_profile_edges_v1().unwrap();
        observed.sort_unstable_by_key(EdgeId::canonical_bytes);
        let mut expected = fixture
            .profiles
            .iter()
            .filter_map(|(edge, profile)| (*profile == TestProfileV1::Collective).then_some(*edge))
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
        assert_eq!(observed, expected);
        assert!(block_cut_free_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn block_cut_free_word_rejects_second_and_three_sample_matched_cyclic_profiles() {
    let fixture = two_block_free_word_fixture_v1(false, false, false, true, false);
    let changed = fixture.cyclic_blocks[0][0];
    let second = polynomial_schedule_v1(&fixture, ScheduleMutationV1::SecondNonconstant(changed));
    assert!(second.collective_profile_edges_v1().is_none());
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
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
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &sample_matched,
        0.0,
    ));
}

#[test]
fn block_cut_free_word_handles_parallel_parent_edges_after_zero_contraction() {
    let fixture = parallel_cut_fixture_v1(true);
    assert_eq!(fixture.zero_edges.len(), 2);
    assert_eq!(fixture.cyclic_blocks[0].len(), 2);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_free_word_cycle_closure_premises_v1(
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
    assert!(decomposition.blocks()[0].is_cyclic());

    let fixture = parallel_cut_fixture_v1(false);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn block_cut_free_word_rejects_binding_and_tolerance_tamper() {
    let fixture = two_block_free_word_fixture_v1(false, false, false, false, true);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        -f64::from_bits(1),
    ));
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        f64::NAN,
    ));

    let other = two_block_free_word_fixture_v1(false, false, false, false, true);
    assert!(!block_cut_free_word_cycle_closure_premises_v1(
        &other.geometry,
        &other.audit,
        other.fixed_face,
        &schedule,
        0.0,
    ));
}
