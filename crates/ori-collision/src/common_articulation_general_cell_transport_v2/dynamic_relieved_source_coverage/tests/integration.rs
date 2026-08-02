use ori_core::analyze_global_flat_foldability;
use ori_foldability::GlobalFlatFoldabilityLimits;

use super::super::super::n33_compact_pair_assignment_fixture_v2::n33_compact_pair_assignment_v2;
use super::super::*;
use super::policy_assertions::{
    assert_endpoint_preflight_limits_and_entry_stops_v2, assert_preflight_limits_and_entry_stops_v2,
};
use super::support::*;
use crate::common_articulation_clearance_v2::test_support::golden_n33_miura_fixture_v2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_public_api_tests::{public_input_v2, public_limits_v2, revalidation_input_v2},
    relief_support::relief_policies_v2,
    support::{n34_fixture_v2, ordinary_fixture_v2},
};
use crate::{
    COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
    CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
    prove_common_articulation_dynamic_general_n_relieved_clearance_v2,
};

#[test]
fn genuine_n33_coverage_fixes_replay_resources_stops_and_fail_closed_boundaries() {
    let fixture = ordinary_fixture_v2(golden_n33_miura_fixture_v2());
    let policies = relief_policies_v2(&fixture);
    let public_limits = public_limits_v2(&fixture);
    let clearance = prove_common_articulation_dynamic_general_n_relieved_clearance_v2(
        public_input_v2(&fixture, &policies, public_limits),
    )
    .expect("genuine N33 Phase 3F certificate");
    assert_phase3f_public_summary_v2(&clearance);

    // These direct Phase 3F assertions used to issue a second N33
    // certificate in its public-API test. Keeping them immediately before
    // consumption preserves that coverage while sharing this golden issuer.
    // Direct Phase 3F revalidation preserves its established resource-error
    // precedence and therefore performs the full proof before rejecting a
    // valid policy drift. Coverage issuance and the final Phase 3H replay
    // retain the distinct expensive success paths.
    for stop in [
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled,
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded,
    ] {
        let replay = clearance.revalidate_with_checkpoint_v2(
            revalidation_input_v2(&fixture, &policies, public_limits),
            || Err(stop),
        );
        assert!(matches!(
            (stop, replay),
            (
                CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled,
                Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::Cancelled)
            ) | (
                CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded,
                Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::DeadlineExceeded)
            )
        ));
    }
    let drifted_phase3f_limits = CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
        max_publication_bytes: public_limits.max_publication_bytes + 1,
        ..public_limits
    };
    let mut phase3f_limit_drift_polls = 0usize;
    assert_eq!(
        clearance.revalidate_with_checkpoint_v2(
            revalidation_input_v2(&fixture, &policies, drifted_phase3f_limits),
            || {
                phase3f_limit_drift_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::CertificateBindingMismatch)
    );
    assert!(phase3f_limit_drift_polls > 100);

    let live_source = LiveGlobalInputV2::for_fixture_v2(&fixture);
    let (variable_count, registry, direction_bits) = n33_compact_pair_assignment_v2();
    // Direction-bit drift is rejected at the compact-source issuer boundary;
    // its dedicated upstream tests exercise that expensive proof once.
    let compact = try_issue_compact_n33_source_v2(
        &fixture,
        &live_source,
        &direction_bits,
        variable_count,
        registry,
    )
    .expect("genuine N33 compact source");
    let source_limits = source_revalidation_limits_v2(&compact);
    let authority = compact
        .revalidate_live_source_v2(live_source.input(&fixture), source_limits)
        .expect("live semantic N33 source authority");
    let limits =
        exact_coverage_limits_v2(&fixture, &clearance, authority.layer_order_snapshot_v2());
    let certificate =
        prove_common_articulation_dynamic_general_n_relieved_source_order_coverage_v2(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageInputV2 {
                clearance,
                live: revalidation_input_v2(&fixture, &policies, public_limits),
                source_authority: &authority,
                limits,
            },
        )
        .expect("genuine N33 source pairs are covered by Phase 3F domain");

    assert_eq!(certificate.actual_block_count_v2(), 33);
    assert_eq!(certificate.material_face_count_v2(), 265);
    assert_eq!(certificate.source_order_pair_count_v2(), 34_980);
    assert!(certificate.all_source_order_pairs_covered_by_relieved_clearance_v2());
    assert_eq!(
        certificate.publication_bytes_v2(),
        limits.max_publication_bytes
    );
    assert_eq!(
        certificate.aggregate_peak_bytes_upper_bound_v2(),
        limits.max_aggregate_peak_bytes
    );
    assert!(!certificate.authorizes_continuous_motion());
    assert!(!certificate.authorizes_collision_clearance());
    assert!(!certificate.authorizes_layer_transport());
    assert!(!certificate.authorizes_project_mutation());
    assert!(!certificate.authorizes_apply());
    assert!(!certificate.authorizes_viewer());
    assert!(!certificate.authorizes_export());
    let debug = format!("{certificate:?}");
    for secret in [
        "source_digest",
        "source_provenance",
        "binding_fingerprint",
        "clearance",
        "registry",
        "supporting_cells",
    ] {
        assert!(!debug.contains(secret), "Debug leaked {secret}");
    }

    // A new sealed handle for the same completely replayed source is semantic
    // identity, not an issuer-pointer mismatch.
    let fresh_authority = compact
        .revalidate_live_source_v2(live_source.input(&fixture), source_limits)
        .expect("fresh semantic-equal source authority");
    assert_preflight_limits_and_entry_stops_v2(
        &certificate,
        &fixture,
        &policies,
        public_limits,
        &fresh_authority,
        limits,
    );
    let drifted_coverage_limits =
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
            max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes + 1,
            ..limits
        };
    let mut phase3g_policy_polls = 0usize;
    assert_eq!(
        certificate.revalidate_with_checkpoint_v2(
            replay_input_v2(
                &fixture,
                &policies,
                public_limits,
                &fresh_authority,
                drifted_coverage_limits,
            ),
            || {
                phase3g_policy_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(phase3g_policy_polls, 1, "Phase 3G outer drift is fail-fast");
    let drifted_clearance_limits = CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
        max_aggregate_peak_bytes: public_limits.max_aggregate_peak_bytes + 1,
        ..public_limits
    };
    let mut phase3f_policy_polls = 0usize;
    assert_eq!(
        certificate.revalidate_with_checkpoint_v2(
            replay_input_v2(
                &fixture,
                &policies,
                drifted_clearance_limits,
                &fresh_authority,
                limits,
            ),
            || {
                phase3f_policy_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(phase3f_policy_polls, 1, "Phase 3F drift is fail-fast");

    let endpoint_limits = exact_endpoint_limits_v2(&certificate);
    let endpoint = prove_common_articulation_dynamic_general_n_closed_dyadic_endpoint_positive_thickness_prerequisite_v2(
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteInputV2 {
            coverage: certificate,
            limits: endpoint_limits,
        },
    )
    .expect("Phase 3G promotes to closed-dyadic-boundary coverage without replay");
    assert_eq!(endpoint.actual_block_count_v2(), 33);
    assert_eq!(endpoint.material_face_count_v2(), 265);
    assert_eq!(endpoint.source_order_pair_count_v2(), 34_980);
    assert_eq!(endpoint.closed_dyadic_domain_boundary_count_v2(), 2);
    assert!(endpoint.both_closed_dyadic_domain_boundaries_covered_by_positive_thickness_v2());
    assert_eq!(
        endpoint.retained_coverage_bytes_v2(),
        endpoint_limits.max_retained_coverage_bytes
    );
    assert_eq!(
        endpoint.promotion_logical_work_v2(),
        endpoint_limits.max_promotion_logical_work
    );
    assert_eq!(
        endpoint.publication_bytes_v2(),
        endpoint_limits.max_publication_bytes
    );
    assert_eq!(
        endpoint.aggregate_peak_bytes_upper_bound_v2(),
        endpoint_limits.max_aggregate_peak_bytes
    );
    assert!(!endpoint.authorizes_continuous_motion());
    assert!(!endpoint.authorizes_collision_clearance());
    assert!(!endpoint.authorizes_layer_transport());
    assert!(!endpoint.authorizes_project_mutation());
    assert!(!endpoint.authorizes_apply());
    assert!(!endpoint.authorizes_viewer());
    assert!(!endpoint.authorizes_export());
    let endpoint_debug = format!("{endpoint:?}");
    for secret in [
        "boundary_coverage",
        "accepted_leaves",
        "binding_fingerprint",
        "clearance",
        "source_digest",
    ] {
        assert!(!endpoint_debug.contains(secret), "Debug leaked {secret}");
    }

    // This replaces the former successful Phase 3G replay above: Phase 3H
    // delegates exactly that replay, so the golden test adds no full proof run.
    let mut full_polls = 0usize;
    endpoint
        .revalidate_with_checkpoint_v2(
            endpoint_replay_input_v2(
                &fixture,
                &policies,
                public_limits,
                &fresh_authority,
                limits,
                endpoint_limits,
            ),
            || {
                full_polls += 1;
                Ok(())
            },
        )
        .expect("fresh semantic-equal source preserves the endpoint prerequisite");
    assert!(full_polls > 100);
    assert_endpoint_preflight_limits_and_entry_stops_v2(
        &endpoint,
        &fixture,
        &policies,
        public_limits,
        &fresh_authority,
        limits,
        endpoint_limits,
    );
    let drifted_endpoint_limits =
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2 {
            max_aggregate_peak_bytes: endpoint_limits.max_aggregate_peak_bytes + 1,
            ..endpoint_limits
        };
    let mut endpoint_policy_polls = 0usize;
    assert_eq!(
        endpoint.revalidate_with_checkpoint_v2(
            endpoint_replay_input_v2(
                &fixture,
                &policies,
                public_limits,
                &fresh_authority,
                limits,
                drifted_endpoint_limits,
            ),
            || {
                endpoint_policy_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch),
        "valid Phase 3H outer policy drift fails before delegated replay"
    );
    assert_eq!(endpoint_policy_polls, 1);
    let drifted_coverage_limits =
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
            max_publication_bytes: limits.max_publication_bytes + 1,
            ..limits
        };
    let mut endpoint_nested_coverage_polls = 0usize;
    assert_eq!(
        endpoint.revalidate_with_checkpoint_v2(
            endpoint_replay_input_v2(
                &fixture,
                &policies,
                public_limits,
                &fresh_authority,
                drifted_coverage_limits,
                endpoint_limits,
            ),
            || {
                endpoint_nested_coverage_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch),
        "nested Phase 3G policy drift fails before delegated replay"
    );
    assert_eq!(endpoint_nested_coverage_polls, 1);
    let drifted_clearance_limits = CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
        max_publication_bytes: public_limits.max_publication_bytes + 1,
        ..public_limits
    };
    let mut endpoint_nested_clearance_polls = 0usize;
    assert_eq!(
        endpoint.revalidate_with_checkpoint_v2(
            endpoint_replay_input_v2(
                &fixture,
                &policies,
                drifted_clearance_limits,
                &fresh_authority,
                limits,
                endpoint_limits,
            ),
            || {
                endpoint_nested_clearance_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch),
        "nested Phase 3F policy drift fails before delegated replay"
    );
    assert_eq!(endpoint_nested_clearance_polls, 1);

    let foreign_live = super::super::super::test_support::small_live_global_input_v2();
    let foreign_report = analyze_global_flat_foldability(
        foreign_live.input(),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("foreign small source report");
    let foreign_authority = foreign_report
        .layer_order_source_authority_v2()
        .expect("foreign sealed source");
    assert_eq!(
        endpoint.revalidate_v2(endpoint_replay_input_v2(
            &fixture,
            &policies,
            public_limits,
            &foreign_authority,
            limits,
            endpoint_limits,
        )),
        Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Coverage(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::SourceBindingMismatch
        ))
    );

    // The existing Phase 3F public-API test supplies the standalone genuine
    // N34 positive. This repository has no genuine N34 source asset, so this
    // retained N33 coverage identity cannot match the N34 live/policy tuple.
    // Phase 3H rejects that tuple at the exact replay-policy boundary and does
    // not manufacture a genuine N34 source authority for the test.
    let n34 = n34_fixture_v2();
    let n34_policies = relief_policies_v2(n34);
    let n34_public_limits = public_limits_v2(n34);
    let n34_limits = CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
        max_blocks: n34.fixture.profile.configured_max_blocks_v2(),
        ..limits
    };
    assert_eq!(
        endpoint.revalidate_v2(endpoint_replay_input_v2(
            n34,
            &n34_policies,
            n34_public_limits,
            &fresh_authority,
            n34_limits,
            endpoint_limits,
        )),
        Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
}

fn assert_phase3f_public_summary_v2(
    clearance: &crate::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
) {
    assert_eq!(
        clearance.model_id_v2(),
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2
    );
    assert_eq!(clearance.actual_block_count_v2(), 33);
    assert_eq!(clearance.total_face_pairs_v2(), 34_980);
    assert_eq!(clearance.ordinary_face_pairs_v2(), 34_256);
    assert_eq!(clearance.shared_hinge_pairs_v2(), 396);
    assert_eq!(clearance.shared_vertex_pairs_v2(), 328);
    assert!(clearance.whole_parent_positive_thickness_proven_v2());
    assert!(!clearance.authorizes_project_mutation());
    assert!(!clearance.authorizes_apply());
    assert!(!clearance.authorizes_viewer());
    assert!(!clearance.authorizes_export());
    let debug = format!("{clearance:?}");
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
