//! Focused Phase 3E whole-parent proof tests.

use super::super::{OrdinaryIntervalStopV2, relief_aggregate::*};
use super::relief_support::{generous_relief_limits_v2, relief_input_v2, relief_policies_v2};
use super::support::{N33, N34, fresh_n33_fixture_v2, n33_fixture_v2, n34_fixture_v2};

#[test]
fn n33_n34_whole_parent_shared_relief_is_exhaustive_and_continuous() {
    for (fixture, block_count, expected) in [
        (n33_fixture_v2(), N33, (396, 328, 196, 724, 6_448)),
        (n34_fixture_v2(), N34, (408, 338, 202, 746, 6_644)),
    ] {
        assert_eq!(fixture.fixture.profile.actual_block_count_v2(), block_count);
        let policies = relief_policies_v2(fixture);
        assert_eq!(policies.hinge.len(), expected.0);
        assert_eq!(policies.vertex.len(), expected.2);
        let evidence = prove_whole_parent_positive_thickness_v2(relief_input_v2(
            fixture,
            &policies,
            generous_relief_limits_v2(fixture),
        ))
        .expect("whole-parent ordinary plus shared relief");
        let (total, ordinary, hinges, vertices, resources) =
            inspect_whole_parent_evidence_for_test_v2(&evidence);
        let debug = format!("{evidence:?}");
        for secret in [
            "issuer_geometry",
            "ordinary_binding",
            "relief_binding",
            "aggregate_binding",
            "shared_pair_digest",
        ] {
            assert!(!debug.contains(secret), "Debug leaked {secret}");
        }
        assert_eq!(hinges, expected.0);
        assert_eq!(vertices, expected.1);
        assert_eq!(resources.shared_pairs, expected.3);
        assert_eq!(resources.rest_carrier_vertices, expected.4);
        assert_eq!(total, ordinary + expected.3);
        assert!(resources.accepted_interval_leaves > 0);
    }
}

#[test]
fn finite_hinge_scope_and_strict_separation_reject_fake_boundaries() {
    // An infinite side strip would ignore axial position. The production
    // finite-segment predicate includes both hinge endpoints and rejects
    // otherwise convincing points behind or beyond the live hinge.
    assert!(finite_hinge_axial_position_for_test_v2(0, 10));
    assert!(finite_hinge_axial_position_for_test_v2(10, 10));
    assert!(!finite_hinge_axial_position_for_test_v2(-1, 10));
    assert!(!finite_hinge_axial_position_for_test_v2(11, 10));

    assert!(!strict_intervals_disjoint_for_test_v2(
        [0.0, 1.0],
        [1.0, 2.0]
    ));
    let after_one = f64::from_bits(1.0_f64.to_bits() + 1);
    assert!(strict_intervals_disjoint_for_test_v2(
        [0.0, 1.0],
        [after_one, 2.0]
    ));
}

#[test]
fn n33_shared_relief_fails_closed_for_policy_binding_geometry_and_stops() {
    let fixture = n33_fixture_v2();
    let policies = relief_policies_v2(fixture);
    let limits = generous_relief_limits_v2(fixture);
    let input = relief_input_v2(fixture, &policies, limits);

    // The exact V1 policy inequality observes binary64 exactly: 6.0 is below
    // 60 * binary64(0.1), while the next representable width is admissible.
    let mut boundary = policies.hinge[0];
    boundary.cutout_width_mm = 6.0;
    assert_eq!(
        validate_hinge_policy_for_test_v2(&input, &boundary),
        Err(ReliefAggregateErrorV2::UnprovenSharedRelief)
    );
    boundary.cutout_width_mm = f64::from_bits(6.0_f64.to_bits() + 1);
    assert_eq!(validate_hinge_policy_for_test_v2(&input, &boundary), Ok(5));
    boundary.bevel_angle_degrees = 0.0;
    assert_eq!(
        validate_hinge_policy_for_test_v2(&input, &boundary),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );
    boundary.bevel_angle_degrees = 1.0;
    boundary.cutout_width_mm = f64::INFINITY;
    assert_eq!(
        validate_hinge_policy_for_test_v2(&input, &boundary),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );

    let mut vertex_boundary = policies.vertex[0].clone();
    let thickness = fixture.fixture.paper.thickness_mm;
    vertex_boundary.cutout_radius_mm = thickness;
    assert_eq!(
        validate_vertex_policy_for_test_v2(&input, &vertex_boundary),
        Ok(2)
    );
    vertex_boundary.cutout_radius_mm = f64::from_bits(thickness.to_bits() - 1);
    assert_eq!(
        validate_vertex_policy_for_test_v2(&input, &vertex_boundary),
        Err(ReliefAggregateErrorV2::UnprovenSharedRelief)
    );

    let mut duplicate_hinge = policies.clone();
    duplicate_hinge.hinge[1] = duplicate_hinge.hinge[0];
    assert_eq!(
        prove_shared_relief_for_test_v2(relief_input_v2(fixture, &duplicate_hinge, limits,)),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );

    let mut wrong_incidence = policies.clone();
    let policy = wrong_incidence
        .vertex
        .iter_mut()
        .max_by_key(|record| record.incident_faces.len())
        .expect("N33 vertex policy");
    assert!(policy.incident_faces.len() > 2);
    policy.incident_faces.pop();
    assert_eq!(
        prove_shared_relief_for_test_v2(relief_input_v2(fixture, &wrong_incidence, limits,)),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );

    // A huge live thickness is finite, but cannot be replayed against the
    // bridge/common-pose tuple issued for the fixture's bit-exact thickness.
    let mut huge = policies.clone();
    for policy in &mut huge.hinge {
        policy.material_thickness_mm = f64::MAX;
    }
    for policy in &mut huge.vertex {
        policy.material_thickness_mm = f64::MAX;
    }
    let mut huge_input = relief_input_v2(fixture, &huge, limits);
    huge_input.ordinary.paper_thickness_mm = f64::MAX;
    assert_eq!(
        prove_shared_relief_for_test_v2(huge_input),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );

    // Same-size A -> fresh A issuer replay: a stale bridge cannot be reused
    // after the live graph tuple has changed away and back structurally.
    let fresh = fresh_n33_fixture_v2();
    let fresh_policies = relief_policies_v2(&fresh);
    let mut stale_bridge =
        relief_input_v2(&fresh, &fresh_policies, generous_relief_limits_v2(&fresh));
    stale_bridge.ordinary.dynamic_closure_bridge = &fixture.bridge;
    assert_eq!(
        prove_shared_relief_for_test_v2(stale_bridge),
        Err(ReliefAggregateErrorV2::InvalidInput)
    );

    let mut poll_count = 0usize;
    prove_shared_relief_with_checkpoint_for_test_v2(input, || {
        poll_count += 1;
        Ok(())
    })
    .expect("successful poll-count replay");
    assert!(poll_count > 1_000);

    let cancel_at = poll_count / 2;
    let mut polls = 0usize;
    assert_eq!(
        prove_shared_relief_with_checkpoint_for_test_v2(input, || {
            polls += 1;
            if polls == cancel_at {
                Err(OrdinaryIntervalStopV2::Cancelled)
            } else {
                Ok(())
            }
        }),
        Err(ReliefAggregateErrorV2::Cancelled)
    );

    let deadline_at = poll_count - 1;
    let mut polls = 0usize;
    assert_eq!(
        prove_shared_relief_with_checkpoint_for_test_v2(input, || {
            polls += 1;
            if polls == deadline_at {
                Err(OrdinaryIntervalStopV2::DeadlineExceeded)
            } else {
                Ok(())
            }
        }),
        Err(ReliefAggregateErrorV2::DeadlineExceeded)
    );

    let mut invalid_at_entry = input;
    invalid_at_entry.limits.max_hash_work = usize::MAX;
    assert!(matches!(
        prove_whole_parent_positive_thickness_with_checkpoint_v2(invalid_at_entry, || {
            Err(OrdinaryIntervalStopV2::Cancelled)
        }),
        Err(ReliefAggregateErrorV2::Cancelled)
    ));
}
