//! The repository has a genuine compact general-N source asset only for N33.
//! N34 coverage is therefore tested fail-closed against that foreign source;
//! no test-only N34 authority is fabricated.

use super::{
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2, coverage_limits_match_v2,
};

#[path = "tests/integration.rs"]
mod integration;
#[path = "tests/pair_scanner.rs"]
mod pair_scanner;
#[path = "tests/phase3i_boundary_configuration.rs"]
mod phase3i_boundary_configuration;
#[path = "tests/phase3j_representation_boundary_pose.rs"]
mod phase3j_representation_boundary_pose;
#[path = "tests/phase3k_canonical_pose.rs"]
mod phase3k_canonical_pose;
#[path = "tests/policy_assertions.rs"]
mod policy_assertions;
#[path = "tests/support.rs"]
mod support;

#[test]
fn every_valid_limit_drift_changes_the_replay_identity() {
    let retained = CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
        max_blocks: 33,
        max_source_retained_bytes: 34,
        max_material_faces: 35,
        max_folded_faces: 36,
        max_overlap_cells: 37,
        max_face_pair_orders: 38,
        max_global_order_faces: 39,
        max_layer_records: 40,
        max_boundary_vertices: 41,
        max_source_logical_work: 42,
        max_publication_bytes: 43,
        max_aggregate_peak_bytes: 44,
    };
    assert!(coverage_limits_match_v2(retained, retained));
    for field in 0..12 {
        let drifted = support::set_limit_v2(
            retained,
            field,
            support::limit_value_v2(retained, field) + 1,
        );
        assert!(
            !coverage_limits_match_v2(retained, drifted),
            "field {field}"
        );
    }
}
