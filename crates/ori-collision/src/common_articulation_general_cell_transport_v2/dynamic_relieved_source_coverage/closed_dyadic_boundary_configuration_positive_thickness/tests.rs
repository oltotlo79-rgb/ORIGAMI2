//! Pure Phase 3I policy tests; the genuine replay stays in the shared N33 test.

use super::*;

#[test]
fn every_valid_outer_limit_drift_changes_replay_identity() {
    let retained = limits_v2();
    assert!(validation::limits_match_v2(retained, retained));
    for field in 0..8 {
        let mut drifted = retained;
        match field {
            0 => drifted.max_blocks += 1,
            1 => drifted.max_hinges += 1,
            2 => drifted.max_schedule_deep_retained_bytes += 1,
            3 => drifted.max_boundary_evidence_logical_work += 1,
            4 => drifted.max_boundary_evidence_workspace_bytes += 1,
            5 => drifted.max_retained_endpoint_prerequisite_bytes += 1,
            6 => drifted.max_publication_bytes += 1,
            7 => drifted.max_aggregate_peak_bytes += 1,
            _ => unreachable!(),
        }
        assert!(
            !validation::limits_match_v2(retained, drifted),
            "field {field}"
        );
    }
}

#[test]
fn outer_limit_value_order_is_complete_and_stable() {
    assert_eq!(
        validation::limit_values_v2(limits_v2()),
        [33, 34, 35, 36, 37, 38, 39, 40]
    );
}

#[test]
fn composition_workspace_is_fixed_and_finite() {
    assert_eq!(COMPOSITION_WORKSPACE_BYTES_V2, 512);
}

#[test]
fn schedule_identity_never_substitutes_for_graph_identity() {
    let schedule = [1; 32];
    let retained_graph = [2; 32];
    assert!(validation::schedule_graph_binding_pair_matches_v2(
        schedule,
        retained_graph,
        schedule,
        retained_graph,
    ));
    assert!(!validation::schedule_graph_binding_pair_matches_v2(
        schedule,
        retained_graph,
        schedule,
        [3; 32],
    ));
}

const fn limits_v2() -> CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2{
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2 {
        max_blocks: 33,
        max_hinges: 34,
        max_schedule_deep_retained_bytes: 35,
        max_boundary_evidence_logical_work: 36,
        max_boundary_evidence_workspace_bytes: 37,
        max_retained_endpoint_prerequisite_bytes: 38,
        max_publication_bytes: 39,
        max_aggregate_peak_bytes: 40,
    }
}
