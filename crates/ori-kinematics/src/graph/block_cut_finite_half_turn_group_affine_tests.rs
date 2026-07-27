use num_rational::BigRational;
use num_traits::Zero;

use super::{
    affine::{ExactHalfTurnAffineV1, enumerate_finite_half_turn_group_v1},
    test_support::*,
};
use crate::graph::{
    block_cut_decomposition::prepare_contracted_block_cut_v1,
    exact_generator_word::CanonicalInfiniteLineV1,
};

fn d3_carriers_v1(reverse: bool) -> Vec<CanonicalInfiniteLineV1> {
    let fixture = d3_fixture_v1(reverse, reverse, 0, false);
    let schedule = polynomial_schedule_v1(&fixture);
    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    let block = decomposition
        .blocks()
        .iter()
        .find(|block| block.is_cyclic())
        .unwrap();
    let mut carriers = block
        .edge_indices()
        .iter()
        .map(|edge| decomposition.active_edges()[*edge].line().clone())
        .collect::<Vec<_>>();
    carriers.sort_unstable();
    carriers.dedup();
    carriers
}

#[test]
fn exact_affine_d3_enumeration_closes_at_order_six() {
    let carriers = d3_carriers_v1(false);
    assert_eq!(carriers.len(), 2);
    let group = enumerate_finite_half_turn_group_v1(
        &carriers,
        256,
        512,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        4096,
    )
    .unwrap();
    assert_eq!(group.order, 6);
    assert_eq!(group.carrier_count, 2);
    assert_eq!(group.products, 12);
    assert_eq!(group.transitions.len(), 12);
    for state in 0..group.order {
        for carrier in 0..group.carrier_count {
            let next = group.transition(state, carrier).unwrap();
            assert_eq!(group.transition(next, carrier), Some(state));
        }
    }
    let mut state = 0usize;
    for carrier in [0usize, 1, 0, 1, 0, 1] {
        state = group.transition(state, carrier).unwrap();
    }
    assert_eq!(state, 0);
}

#[test]
fn exact_affine_uses_translation_and_is_storage_orientation_invariant() {
    let normal = d3_carriers_v1(false);
    let reversed = d3_carriers_v1(true);
    assert_eq!(normal, reversed);
    let common_center = [3, 5, 7].map(|value| BigRational::from_integer(value.into()));
    for carrier in &normal {
        let transform = ExactHalfTurnAffineV1::from_line(carrier).unwrap();
        assert!(
            transform
                .translation()
                .iter()
                .any(|component| !component.is_zero())
        );
        assert_eq!(
            transform.right_product(&transform),
            ExactHalfTurnAffineV1::identity()
        );
        assert_eq!(transform.apply_point(&common_center), common_center);
    }
}

#[test]
fn exact_affine_group_bounds_are_exact_and_fail_one_short() {
    let carriers = d3_carriers_v1(false);
    let reference = enumerate_finite_half_turn_group_v1(
        &carriers,
        256,
        512,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        4096,
    )
    .unwrap();
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            4096,
        )
        .is_some()
    );
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order - 1,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            4096,
        )
        .is_none()
    );
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products - 1,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            4096,
        )
        .is_none()
    );
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits - 1,
            reference.exact_work_bits,
            4096,
        )
        .is_none()
    );
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits - 1,
            4096,
        )
        .is_none()
    );

    let mut rejected_component_bits = 0u64;
    let mut accepted_component_bits = 4096u64;
    while rejected_component_bits + 1 < accepted_component_bits {
        let candidate = (rejected_component_bits + accepted_component_bits) / 2;
        if enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            candidate,
        )
        .is_some()
        {
            accepted_component_bits = candidate;
        } else {
            rejected_component_bits = candidate;
        }
    }
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            accepted_component_bits,
        )
        .is_some()
    );
    assert!(
        enumerate_finite_half_turn_group_v1(
            &carriers,
            reference.order,
            reference.products,
            reference.exact_storage_bits,
            reference.exact_work_bits,
            accepted_component_bits - 1,
        )
        .is_none()
    );
}
