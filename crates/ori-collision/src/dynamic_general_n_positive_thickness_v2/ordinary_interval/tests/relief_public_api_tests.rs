//! Focused tests for the additive direct public certificate facade.

use std::mem::size_of;

use crate::{
    COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2,
    CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
    CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
    CommonArticulationDynamicGeneralNRelievedClearanceInputV2,
    CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2,
    CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
    prove_common_articulation_dynamic_general_n_relieved_clearance_v2,
    prove_common_articulation_dynamic_general_n_relieved_clearance_with_checkpoint_v2,
};

use super::super::relief_aggregate::ReliefAggregateLimitsV2;
use super::super::{OrdinaryIntervalLimitsV2, public_adapter::DirectClearanceEvidenceV2};
use super::relief_support::{ReliefFixtureInputV2, generous_relief_limits_v2, relief_policies_v2};
use super::support::{N34, OrdinaryFixtureV2, n33_fixture_v2, n34_fixture_v2};

#[test]
fn n34_issues_without_a_pair_registry() {
    let fixture = n34_fixture_v2();
    let policies = relief_policies_v2(fixture);
    let limits = public_limits_v2(fixture);
    let certificate = prove_common_articulation_dynamic_general_n_relieved_clearance_v2(
        public_input_v2(fixture, &policies, limits),
    )
    .expect("direct N34 relieved-clearance certificate");
    assert_public_summary_v2(&certificate, N34, (37_128, 36_382, 408, 338));
    certificate
        .revalidate_v2(revalidation_input_v2(fixture, &policies, limits))
        .expect("same-live N34 public replay");
}

#[test]
fn outer_profile_memory_and_stop_policies_fail_closed_before_publication() {
    let fixture = n33_fixture_v2();
    let policies = relief_policies_v2(fixture);
    let limits = public_limits_v2(fixture);

    let mut above_profile = limits;
    above_profile.max_blocks += 1;
    assert!(matches!(
        prove_common_articulation_dynamic_general_n_relieved_clearance_v2(public_input_v2(
            fixture,
            &policies,
            above_profile,
        )),
        Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::ResourceLimit)
    ));

    for one_short in [
        CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
            max_shared_pair_registry_bytes: limits.max_shared_pair_registry_bytes - 1,
            ..limits
        },
        CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
            max_publication_bytes: limits.max_publication_bytes - 1,
            ..limits
        },
        CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
            max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes - 1,
            ..limits
        },
    ] {
        assert!(matches!(
            prove_common_articulation_dynamic_general_n_relieved_clearance_v2(public_input_v2(
                fixture, &policies, one_short,
            )),
            Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::ResourceLimit)
        ));
    }

    assert!(matches!(
        prove_common_articulation_dynamic_general_n_relieved_clearance_with_checkpoint_v2(
            public_input_v2(fixture, &policies, limits),
            || Err(CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled),
        ),
        Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::Cancelled)
    ));
    assert!(matches!(
        prove_common_articulation_dynamic_general_n_relieved_clearance_with_checkpoint_v2(
            public_input_v2(fixture, &policies, limits),
            || Err(CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded),
        ),
        Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::DeadlineExceeded)
    ));
    assert_eq!(
        size_of::<crate::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2>(),
        size_of::<DirectClearanceEvidenceV2>(),
        "transparent certificate publication has no uncharged shell"
    );
}

fn assert_public_summary_v2(
    certificate: &crate::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    blocks: usize,
    counts: (usize, usize, usize, usize),
) {
    assert_eq!(
        certificate.model_id_v2(),
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2
    );
    assert_eq!(certificate.actual_block_count_v2(), blocks);
    assert_eq!(certificate.total_face_pairs_v2(), counts.0);
    assert_eq!(certificate.ordinary_face_pairs_v2(), counts.1);
    assert_eq!(certificate.shared_hinge_pairs_v2(), counts.2);
    assert_eq!(certificate.shared_vertex_pairs_v2(), counts.3);
    assert!(certificate.whole_parent_positive_thickness_proven_v2());
    assert!(!certificate.authorizes_project_mutation());
    assert!(!certificate.authorizes_apply());
    assert!(!certificate.authorizes_viewer());
    assert!(!certificate.authorizes_export());
    let debug = format!("{certificate:?}");
    for secret in [
        "issuer_geometry",
        "adapter_binding",
        "aggregate_binding",
        "shared_pair_digest",
        "hinge_policies",
        "vertex_policies",
    ] {
        assert!(!debug.contains(secret), "Debug leaked {secret}");
    }
}

pub(crate) fn public_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
) -> CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'a> {
    CommonArticulationDynamicGeneralNRelievedClearanceInputV2 {
        geometry: &fixture.fixture.geometry,
        audit: &fixture.fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.fixture.parent_fixed_face,
        parent_schedule: &fixture.schedule,
        decomposition: &fixture.fixture.decomposition,
        common_pose: &fixture.common_pose,
        profile: &fixture.fixture.profile,
        dynamic_closure_bridge: &fixture.bridge,
        paper_thickness_mm: fixture.fixture.paper.thickness_mm,
        closure_tolerance: fixture.fixture.closure_tolerance,
        hinge_policies: &policies.hinge,
        vertex_policies: &policies.vertex,
        limits,
    }
}

pub(crate) fn revalidation_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
) -> CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'a> {
    let input = public_input_v2(fixture, policies, limits);
    CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2 {
        geometry: input.geometry,
        audit: input.audit,
        pose: input.pose,
        parent_fixed_face: input.parent_fixed_face,
        parent_schedule: input.parent_schedule,
        decomposition: input.decomposition,
        common_pose: input.common_pose,
        profile: input.profile,
        dynamic_closure_bridge: input.dynamic_closure_bridge,
        paper_thickness_mm: input.paper_thickness_mm,
        closure_tolerance: input.closure_tolerance,
        hinge_policies: input.hinge_policies,
        vertex_policies: input.vertex_policies,
        limits: input.limits,
    }
}

pub(crate) fn public_limits_v2(
    fixture: &OrdinaryFixtureV2,
) -> CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
    let ordinary = super::support::strict_limits_v2(fixture);
    let relief = generous_relief_limits_v2(fixture);
    let registry_bytes =
        ordinary.max_excluded_shared_pairs * size_of::<super::super::OrdinaryIntervalFacePairV2>();
    let publication_bytes = size_of::<DirectClearanceEvidenceV2>();
    let aggregate_peak_bytes = registry_bytes + relief.max_aggregate_peak_bytes + publication_bytes;
    CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
        max_blocks: fixture.fixture.profile.configured_max_blocks_v2(),
        max_shared_pair_registry_bytes: registry_bytes,
        max_publication_bytes: publication_bytes,
        max_aggregate_peak_bytes: aggregate_peak_bytes,
        ordinary: public_ordinary_limits_v2(ordinary),
        relief: public_relief_limits_v2(relief),
    }
}

fn public_ordinary_limits_v2(
    limits: OrdinaryIntervalLimitsV2,
) -> CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2 {
    CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2 {
        max_faces: limits.max_faces,
        max_hinges: limits.max_hinges,
        max_boundary_vertex_occurrences: limits.max_boundary_vertex_occurrences,
        max_excluded_shared_pairs: limits.max_excluded_shared_pairs,
        max_shared_feature_membership_tests: limits.max_shared_feature_membership_tests,
        max_collision_depth: limits.max_collision_depth,
        max_collision_leaves: limits.max_collision_leaves,
        schedule_limits: limits.schedule_limits,
        max_bridge_retained_bytes: limits.max_bridge_retained_bytes,
        max_bridge_revalidation_peak_bytes: limits.max_bridge_revalidation_peak_bytes,
        max_schedule_retained_bytes: limits.max_schedule_retained_bytes,
        max_session_shell_bytes: limits.max_session_shell_bytes,
        max_schedule_evaluation_workspace_bytes: limits.max_schedule_evaluation_workspace_bytes,
        max_bridge_partition_search_work_per_node: limits.max_bridge_partition_search_work_per_node,
        max_interval_transform_work_per_node: limits.max_interval_transform_work_per_node,
        max_interval_registry_validation_work_per_node: limits
            .max_interval_registry_validation_work_per_node,
        max_interval_registry_sort_comparisons_per_node: limits
            .max_interval_registry_sort_comparisons_per_node,
        max_interval_registry_workspace_bytes: limits.max_interval_registry_workspace_bytes,
        max_interval_registry_retained_bytes: limits.max_interval_registry_retained_bytes,
        max_ordinary_pair_node_tests: limits.max_ordinary_pair_node_tests,
        max_logical_work: limits.max_logical_work,
        max_temporary_bytes: limits.max_temporary_bytes,
        max_publication_bytes: limits.max_publication_bytes,
        max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes,
    }
}

fn public_relief_limits_v2(
    limits: ReliefAggregateLimitsV2,
) -> CommonArticulationDynamicGeneralNReliefAggregateLimitsV2 {
    CommonArticulationDynamicGeneralNReliefAggregateLimitsV2 {
        max_hinge_policy_records: limits.max_hinge_policy_records,
        max_vertex_policy_records: limits.max_vertex_policy_records,
        max_vertex_incident_face_occurrences: limits.max_vertex_incident_face_occurrences,
        max_shared_pairs: limits.max_shared_pairs,
        max_pair_membership_tests: limits.max_pair_membership_tests,
        max_pair_hinge_tests: limits.max_pair_hinge_tests,
        max_scope_and_policy_validation_work: limits.max_scope_and_policy_validation_work,
        max_convexity_segment_tests: limits.max_convexity_segment_tests,
        max_rest_carrier_vertices: limits.max_rest_carrier_vertices,
        max_exact_clip_operations: limits.max_exact_clip_operations,
        max_sqrt_calls: limits.max_sqrt_calls,
        max_sqrt_operations_per_call: limits.max_sqrt_operations_per_call,
        max_exact_value_bits: limits.max_exact_value_bits,
        max_exact_scratch_bytes: limits.max_exact_scratch_bytes,
        max_collision_depth: limits.max_collision_depth,
        max_collision_leaves: limits.max_collision_leaves,
        max_shared_pair_node_tests: limits.max_shared_pair_node_tests,
        max_axis_projection_work: limits.max_axis_projection_work,
        max_carrier_conversion_work: limits.max_carrier_conversion_work,
        max_hash_work: limits.max_hash_work,
        max_logical_work: limits.max_logical_work,
        max_temporary_bytes: limits.max_temporary_bytes,
        max_publication_bytes: limits.max_publication_bytes,
        max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes,
    }
}
