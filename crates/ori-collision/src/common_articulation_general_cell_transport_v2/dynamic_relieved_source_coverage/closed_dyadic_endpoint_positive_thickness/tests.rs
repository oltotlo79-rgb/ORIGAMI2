//! Pure Phase 3H policy tests; the genuine N33 replay stays in the existing
//! Phase 3G integration test so this module adds no expensive proof run.

use super::*;

#[test]
fn every_valid_limit_drift_changes_the_replay_identity() {
    let retained =
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2 {
            max_blocks: 33,
            max_retained_coverage_bytes: 34,
            max_promotion_logical_work: 35,
            max_promotion_workspace_bytes: 36,
            max_publication_bytes: 37,
            max_aggregate_peak_bytes: 38,
        };
    assert!(endpoint_limits_match_v2(retained, retained));
    for field in 0..6 {
        let mut drifted = retained;
        match field {
            0 => drifted.max_blocks += 1,
            1 => drifted.max_retained_coverage_bytes += 1,
            2 => drifted.max_promotion_logical_work += 1,
            3 => drifted.max_promotion_workspace_bytes += 1,
            4 => drifted.max_publication_bytes += 1,
            5 => drifted.max_aggregate_peak_bytes += 1,
            _ => unreachable!(),
        }
        assert!(
            !endpoint_limits_match_v2(retained, drifted),
            "field {field}"
        );
    }
}

#[test]
fn limit_value_order_is_complete_and_stable() {
    let limits =
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2 {
            max_blocks: 1,
            max_retained_coverage_bytes: 2,
            max_promotion_logical_work: 3,
            max_promotion_workspace_bytes: 4,
            max_publication_bytes: 5,
            max_aggregate_peak_bytes: 6,
        };
    assert_eq!(
        validation::endpoint_limit_values_v2(limits),
        [1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn fixed_promotion_work_contract_is_the_sum_of_documented_categories() {
    assert_eq!(PROMOTION_LIMIT_POLICY_WORK_V2, 19);
    assert_eq!(PROMOTION_RESOURCE_WORK_V2, 11);
    assert_eq!(PROMOTION_THEOREM_WORK_V2, 6);
    assert_eq!(PROMOTION_BINDING_WORK_V2, 25);
    assert_eq!(PROMOTION_REPLAY_POLICY_WORK_V2, 77);
    assert_eq!(PROMOTION_LOGICAL_WORK_V2, 138);
}

#[test]
fn boundary_predicate_requires_exactly_one_leaf_in_each_independent_partition() {
    let exact = ClosedDyadicDomainBoundaryCoverageV2 {
        ordinary_lower_accepted_leaves: 1,
        ordinary_upper_accepted_leaves: 1,
        shared_relief_lower_accepted_leaves: 1,
        shared_relief_upper_accepted_leaves: 1,
    };
    assert!(exact.is_complete_v2());
    for field in 0..4 {
        for invalid in [0, 2] {
            let mut drifted = exact;
            match field {
                0 => drifted.ordinary_lower_accepted_leaves = invalid,
                1 => drifted.ordinary_upper_accepted_leaves = invalid,
                2 => drifted.shared_relief_lower_accepted_leaves = invalid,
                3 => drifted.shared_relief_upper_accepted_leaves = invalid,
                _ => unreachable!(),
            }
            assert!(!drifted.is_complete_v2(), "field {field} accepts {invalid}");
        }
    }
}
