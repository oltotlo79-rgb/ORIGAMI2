//! Phase 3E resource-contract tests without repeated collision partitions.

use super::super::relief_aggregate::*;
use super::relief_support::{
    generous_relief_limits_v2, relief_input_v2, relief_policies_v2, strict_relief_limits_v2,
};
use super::support::{N33, n33_fixture_v2};

fn n33_charged_resources_v2(input: ReliefAggregateInputV2<'_>) -> ReliefAggregateResourcesV2 {
    let evidence =
        prove_whole_parent_positive_thickness_v2(input).expect("generous N33 whole-parent proof");
    let (_, _, _, _, resources) = inspect_whole_parent_evidence_for_test_v2(&evidence);
    assert!(resources.shared_pairs > 0);
    assert!(resources.sqrt_calls > 0);
    assert!(resources.aggregate_peak_bytes > 0);
    resources
}

fn exact_n33_limits_v2(
    generous: ReliefAggregateLimitsV2,
    resources: ReliefAggregateResourcesV2,
) -> ReliefAggregateLimitsV2 {
    strict_relief_limits_v2(generous, resources)
}

fn preflight_n33_v2(
    base: ReliefAggregateInputV2<'_>,
    limits: ReliefAggregateLimitsV2,
) -> Result<(), ReliefAggregateErrorV2> {
    preflight_whole_parent_for_test_v2(ReliefAggregateInputV2 { limits, ..base })
}

fn assert_resource_limit_v2(
    base: ReliefAggregateInputV2<'_>,
    name: &str,
    limits: ReliefAggregateLimitsV2,
) {
    assert_eq!(
        preflight_n33_v2(base, limits),
        Err(ReliefAggregateErrorV2::ResourceLimit),
        "{name} must fail in the resource-only whole-parent preflight",
    );
}

fn one_short_v2(value: usize, name: &str) -> usize {
    value.checked_sub(1).expect(name)
}

#[test]
fn n33_resource_contract_exact_replay_and_one_short_preflights() {
    let fixture = n33_fixture_v2();
    assert_eq!(fixture.fixture.profile.actual_block_count_v2(), N33);
    let policies = relief_policies_v2(fixture);
    let generous = generous_relief_limits_v2(fixture);
    let base = relief_input_v2(fixture, &policies, generous);
    let expected = n33_charged_resources_v2(base);
    let exact = exact_n33_limits_v2(generous, expected);
    assert_eq!(preflight_n33_v2(base, exact), Ok(()));

    let replay = prove_whole_parent_positive_thickness_v2(ReliefAggregateInputV2 {
        limits: exact,
        ..base
    })
    .expect("exact charged N33 caps must replay");
    let (_, _, _, _, observed) = inspect_whole_parent_evidence_for_test_v2(&replay);
    assert_eq!(observed, expected);

    type Setter = fn(&mut ReliefAggregateLimitsV2, usize);
    let cases: [(&str, usize, Setter); 18] = [
        (
            "hinge policy records",
            expected.hinge_policy_records,
            |limits, value| limits.max_hinge_policy_records = value,
        ),
        (
            "vertex policy records",
            expected.vertex_policy_records,
            |limits, value| limits.max_vertex_policy_records = value,
        ),
        (
            "vertex incident face occurrences",
            expected.vertex_incident_face_occurrences,
            |limits, value| limits.max_vertex_incident_face_occurrences = value,
        ),
        ("shared pairs", expected.shared_pairs, |limits, value| {
            limits.max_shared_pairs = value
        }),
        (
            "pair membership tests",
            expected.pair_membership_tests,
            |limits, value| limits.max_pair_membership_tests = value,
        ),
        (
            "pair hinge tests",
            expected.pair_hinge_tests,
            |limits, value| limits.max_pair_hinge_tests = value,
        ),
        (
            "scope and policy validation work",
            expected.scope_and_policy_validation_work,
            |limits, value| limits.max_scope_and_policy_validation_work = value,
        ),
        (
            "convexity segment tests",
            expected.convexity_segment_tests,
            |limits, value| limits.max_convexity_segment_tests = value,
        ),
        (
            "rest carrier vertices",
            expected.rest_carrier_vertices,
            |limits, value| limits.max_rest_carrier_vertices = value,
        ),
        (
            "exact clip operations",
            expected.exact_clip_operations,
            |limits, value| limits.max_exact_clip_operations = value,
        ),
        ("sqrt calls", expected.sqrt_calls, |limits, value| {
            limits.max_sqrt_calls = value
        }),
        (
            "exact scratch bytes",
            expected.exact_scratch_bytes,
            |limits, value| limits.max_exact_scratch_bytes = value,
        ),
        (
            "shared pair node tests",
            expected.shared_pair_node_tests,
            |limits, value| limits.max_shared_pair_node_tests = value,
        ),
        (
            "axis projection work",
            expected.axis_projection_work,
            |limits, value| limits.max_axis_projection_work = value,
        ),
        (
            "carrier conversion work",
            expected.carrier_conversion_work,
            |limits, value| limits.max_carrier_conversion_work = value,
        ),
        ("hash work", expected.hash_work, |limits, value| {
            limits.max_hash_work = value
        }),
        ("logical work", expected.logical_work, |limits, value| {
            limits.max_logical_work = value
        }),
        (
            "publication bytes",
            expected.publication_bytes,
            |limits, value| limits.max_publication_bytes = value,
        ),
    ];
    for (name, charged, setter) in cases {
        let mut limits = exact;
        setter(&mut limits, one_short_v2(charged, name));
        assert_resource_limit_v2(base, name, limits);
    }
    for (name, charged, setter) in [
        (
            "temporary bytes",
            expected.temporary_bytes,
            (|limits: &mut ReliefAggregateLimitsV2, value| limits.max_temporary_bytes = value)
                as fn(&mut ReliefAggregateLimitsV2, usize),
        ),
        (
            "aggregate peak bytes",
            expected.aggregate_peak_bytes,
            (|limits: &mut ReliefAggregateLimitsV2, value| limits.max_aggregate_peak_bytes = value)
                as fn(&mut ReliefAggregateLimitsV2, usize),
        ),
    ] {
        let mut limits = exact;
        setter(&mut limits, one_short_v2(charged, name));
        assert_resource_limit_v2(base, name, limits);
    }
}

#[test]
fn scalar_hard_boundary_and_profile_derived_limits_are_rejected_as_resource_limits() {
    type Setter = fn(&mut ReliefAggregateLimitsV2, usize);
    let scalar_limits: [(&str, Setter); 23] = [
        ("hinge policy records", |limits, value| {
            limits.max_hinge_policy_records = value
        }),
        ("vertex policy records", |limits, value| {
            limits.max_vertex_policy_records = value
        }),
        ("vertex incident face occurrences", |limits, value| {
            limits.max_vertex_incident_face_occurrences = value
        }),
        ("shared pairs", |limits, value| {
            limits.max_shared_pairs = value
        }),
        ("pair membership tests", |limits, value| {
            limits.max_pair_membership_tests = value
        }),
        ("pair hinge tests", |limits, value| {
            limits.max_pair_hinge_tests = value
        }),
        ("scope and policy work", |limits, value| {
            limits.max_scope_and_policy_validation_work = value
        }),
        ("convexity segment tests", |limits, value| {
            limits.max_convexity_segment_tests = value
        }),
        ("rest carrier vertices", |limits, value| {
            limits.max_rest_carrier_vertices = value
        }),
        ("exact clip operations", |limits, value| {
            limits.max_exact_clip_operations = value
        }),
        ("sqrt calls", |limits, value| limits.max_sqrt_calls = value),
        ("sqrt operations per call", |limits, value| {
            limits.max_sqrt_operations_per_call = value
        }),
        ("exact value bits", |limits, value| {
            limits.max_exact_value_bits = value
        }),
        ("exact scratch bytes", |limits, value| {
            limits.max_exact_scratch_bytes = value
        }),
        ("collision leaves", |limits, value| {
            limits.max_collision_leaves = value
        }),
        ("shared pair node tests", |limits, value| {
            limits.max_shared_pair_node_tests = value
        }),
        ("axis projection work", |limits, value| {
            limits.max_axis_projection_work = value
        }),
        ("carrier conversion work", |limits, value| {
            limits.max_carrier_conversion_work = value
        }),
        ("hash work", |limits, value| limits.max_hash_work = value),
        ("logical work", |limits, value| {
            limits.max_logical_work = value
        }),
        ("temporary bytes", |limits, value| {
            limits.max_temporary_bytes = value
        }),
        ("publication bytes", |limits, value| {
            limits.max_publication_bytes = value
        }),
        ("aggregate peak bytes", |limits, value| {
            limits.max_aggregate_peak_bytes = value
        }),
    ];
    let fixture = n33_fixture_v2();
    let generous = generous_relief_limits_v2(fixture);
    let policies = relief_policies_v2(fixture);
    let base = relief_input_v2(fixture, &policies, generous);
    for value in [0, usize::MAX] {
        for (name, setter) in scalar_limits {
            let mut limits = generous;
            setter(&mut limits, value);
            assert_resource_limit_v2(base, &format!("{name}={value}"), limits);
        }
    }

    for (name, limits) in [
        (
            "collision depth zero",
            ReliefAggregateLimitsV2 {
                max_collision_depth: 0,
                ..generous
            },
        ),
        (
            "collision depth 64",
            ReliefAggregateLimitsV2 {
                max_collision_depth: 64,
                ..generous
            },
        ),
        (
            "collision leaves above hard maximum",
            ReliefAggregateLimitsV2 {
                max_collision_leaves: 65_537,
                ..generous
            },
        ),
        (
            "sqrt operations above hard maximum",
            ReliefAggregateLimitsV2 {
                max_sqrt_operations_per_call: 20_001,
                ..generous
            },
        ),
        (
            "exact value bits above hard maximum",
            ReliefAggregateLimitsV2 {
                max_exact_value_bits: 32_769,
                ..generous
            },
        ),
        (
            "shared pairs above hard maximum",
            ReliefAggregateLimitsV2 {
                max_shared_pairs: 1_048_577,
                ..generous
            },
        ),
    ] {
        assert_resource_limit_v2(base, name, limits);
    }

    let profile = fixture.fixture.profile.maximum_v2();
    let max_vertex_occurrences = profile
        .face_count_v2()
        .checked_mul(4)
        .expect("fixture profile vertex occurrence bound");
    let cases = [
        (
            "hinge record cap above profile maximum",
            ReliefAggregateLimitsV2 {
                max_hinge_policy_records: profile
                    .hinge_count_v2()
                    .checked_add(1)
                    .expect("fixture hinge profile cap"),
                ..generous
            },
        ),
        (
            "vertex record cap above profile maximum",
            ReliefAggregateLimitsV2 {
                max_vertex_policy_records: max_vertex_occurrences
                    .checked_add(1)
                    .expect("fixture vertex profile cap"),
                ..generous
            },
        ),
        (
            "vertex occurrence cap above profile maximum",
            ReliefAggregateLimitsV2 {
                max_vertex_incident_face_occurrences: max_vertex_occurrences
                    .checked_add(1)
                    .expect("fixture vertex occurrence profile cap"),
                ..generous
            },
        ),
    ];
    for (name, limits) in cases {
        assert_resource_limit_v2(base, name, limits);
    }
}
