//! Phase 3I assertions shared by the one genuine N33 integration test.
//!
//! This module deliberately defines no `#[test]`; its helper consumes Phase
//! 3H once, and the existing integration test remains the sole heavy proof.

use ori_core::analyze_global_flat_foldability;
use ori_foldability::{GlobalFlatFoldabilityLimits, GlobalFlatLayerOrderSourceAuthorityV2};

use super::super::*;
use super::policy_assertions::assert_boundary_configuration_preflight_limits_and_entry_stops_v2;
use super::support::*;
use crate::CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2;
use crate::common_articulation_clearance_v2::test_support::golden_n33_miura_fixture_v2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_public_api_tests::public_limits_v2,
    relief_support::{ReliefFixtureInputV2, relief_policies_v2},
    support::{OrdinaryFixtureV2, n34_fixture_v2, nonstationary_schedule_for_fixed_face_v2},
};

#[allow(clippy::too_many_arguments)]
pub(super) fn assert_phase3i_boundary_configuration_v2<'a>(
    endpoint: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    fresh_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) {
    assert_eq!(
        endpoint.schedule_binding_fingerprint_v2(),
        fixture.schedule.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        endpoint.graph_binding_fingerprint_v1(),
        fixture.schedule.graph_binding_fingerprint_v1()
    );
    assert!(endpoint.matches_geometry_instance_v2(&fixture.fixture.geometry));
    let schedule_limits = public_limits.ordinary.schedule_limits;
    let limits =
        exact_boundary_configuration_limits_v2(&endpoint, &fixture.schedule, schedule_limits);
    let boundary = prove_common_articulation_dynamic_general_n_closed_dyadic_boundary_configuration_positive_thickness_prerequisite_v2(
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteInputV2 {
            geometry: &fixture.fixture.geometry,
            schedule: &fixture.schedule,
            schedule_limits,
            endpoint_prerequisite: endpoint,
            limits,
        },
    )
    .expect("Phase 3H and kinematics boundaries share one exact schedule and geometry instance");
    assert_eq!(boundary.actual_block_count_v2(), 33);
    assert_eq!(boundary.material_face_count_v2(), 265);
    assert_eq!(boundary.source_order_pair_count_v2(), 34_980);
    assert_eq!(boundary.hinge_count_v2(), limits.max_hinges);
    assert_eq!(boundary.closed_dyadic_boundary_configuration_count_v2(), 2);
    assert!(boundary.both_closed_dyadic_boundary_configurations_have_positive_thickness_v2());
    assert_eq!(
        boundary.retained_endpoint_prerequisite_bytes_v2(),
        limits.max_retained_endpoint_prerequisite_bytes
    );
    assert_eq!(
        boundary.schedule_deep_retained_bytes_upper_bound_v2(),
        limits.max_schedule_deep_retained_bytes
    );
    assert_eq!(
        boundary.boundary_evidence_logical_work_v2(),
        limits.max_boundary_evidence_logical_work
    );
    assert_eq!(
        boundary.boundary_evidence_workspace_bytes_upper_bound_v2(),
        limits.max_boundary_evidence_workspace_bytes
    );
    assert_eq!(
        boundary.publication_bytes_v2(),
        limits.max_publication_bytes
    );
    assert_eq!(
        boundary.aggregate_peak_bytes_upper_bound_v2(),
        limits.max_aggregate_peak_bytes
    );
    assert!(!boundary.authorizes_continuous_motion());
    assert!(!boundary.authorizes_collision_clearance());
    assert!(!boundary.authorizes_layer_transport());
    assert!(!boundary.authorizes_project_mutation());
    assert!(!boundary.authorizes_apply());
    assert!(!boundary.authorizes_viewer());
    assert!(!boundary.authorizes_export());
    let debug = format!("{boundary:?}");
    for secret in [
        "issuer_geometry",
        "endpoint_prerequisite",
        "boundary_evidence",
        "schedule_binding",
        "graph_binding",
        "binding_fingerprint",
        "accepted_leaves",
    ] {
        assert!(!debug.contains(secret), "Debug leaked {secret}");
    }

    assert_boundary_configuration_preflight_limits_and_entry_stops_v2(
        &boundary,
        fixture,
        policies,
        public_limits,
        fresh_authority,
        coverage_limits,
        endpoint_limits,
        schedule_limits,
        limits,
    );

    let drifted_limits = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2 {
        max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes + 1,
        ..limits
    };
    let mut polls = 0usize;
    assert_eq!(
        boundary.revalidate_with_checkpoint_v2(
            boundary_configuration_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                schedule_limits,
                drifted_limits,
            ),
            || {
                polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(polls, 1);

    let drifted_schedule_limits = ori_kinematics::CycleScheduleLimitsV1 {
        max_work: schedule_limits.max_work + 1,
        ..schedule_limits
    };
    let mut polls = 0usize;
    assert_eq!(
        boundary.revalidate_with_checkpoint_v2(
            boundary_configuration_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                drifted_schedule_limits,
                limits,
            ),
            || {
                polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(polls, 1);

    let drifted_endpoint_limits = CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2 {
        max_aggregate_peak_bytes: endpoint_limits.max_aggregate_peak_bytes + 1,
        ..endpoint_limits
    };
    let mut polls = 0usize;
    assert_eq!(
        boundary.revalidate_with_checkpoint_v2(
            boundary_configuration_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                drifted_endpoint_limits,
                schedule_limits,
                limits,
            ),
            || {
                polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(polls, 1);

    assert_foreign_geometry_and_graph_fail_fast_v2(
        &boundary,
        fixture,
        policies,
        public_limits,
        fresh_authority,
        coverage_limits,
        endpoint_limits,
        schedule_limits,
        limits,
    );

    // This replaces the former successful Phase 3H replay. Phase 3I delegates
    // that exact replay, so the golden test retains one full collision proof.
    let mut full_polls = 0usize;
    boundary
        .revalidate_with_checkpoint_v2(
            boundary_configuration_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                schedule_limits,
                limits,
            ),
            || {
                full_polls += 1;
                Ok(())
            },
        )
        .expect("fresh semantic-equal source preserves the joined boundary prerequisite");
    assert!(full_polls > 100);

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
        boundary.revalidate_v2(boundary_configuration_replay_input_v2(
            fixture,
            policies,
            public_limits,
            &foreign_authority,
            coverage_limits,
            endpoint_limits,
            schedule_limits,
            limits,
        )),
        Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::EndpointPositiveThickness(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Coverage(
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::SourceBindingMismatch
            )
        ))
    );

    // No genuine N34 source asset exists, so the retained N33 identity rejects
    // the N34 live/policy tuple without fabricating authority.
    let n34 = n34_fixture_v2();
    let n34_policies = relief_policies_v2(n34);
    let n34_public_limits = public_limits_v2(n34);
    let n34_limits = CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
        max_blocks: n34.fixture.profile.configured_max_blocks_v2(),
        ..coverage_limits
    };
    assert_eq!(
        boundary.revalidate_v2(boundary_configuration_replay_input_v2(
            n34,
            &n34_policies,
            n34_public_limits,
            fresh_authority,
            n34_limits,
            endpoint_limits,
            schedule_limits,
            limits,
        )),
        Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_foreign_geometry_and_graph_fail_fast_v2<'a>(
    boundary: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
) {
    {
        let fresh = golden_n33_miura_fixture_v2();
        assert_eq!(
            fresh.geometry.fold_model_fingerprint_v1(),
            fixture.fixture.geometry.fold_model_fingerprint_v1()
        );
        let mut replay = boundary_configuration_replay_input_v2(
            fixture,
            policies,
            public_limits,
            authority,
            coverage_limits,
            endpoint_limits,
            schedule_limits,
            limits,
        );
        replay.geometry = &fresh.geometry;
        let mut polls = 0usize;
        assert_eq!(
            boundary.revalidate_with_checkpoint_v2(replay, || {
                polls += 1;
                Ok(())
            }),
            Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
        );
        assert_eq!(polls, 1);
    }

    {
        let alternate_fixed_face = fixture
            .fixture
            .geometry
            .face_ids()
            .iter()
            .copied()
            .find(|face| *face != fixture.fixture.parent_fixed_face)
            .expect("N33 has an alternate fixed face");
        let foreign =
            nonstationary_schedule_for_fixed_face_v2(&fixture.fixture, alternate_fixed_face);
        assert_eq!(
            foreign.certificate_binding_fingerprint_v2(),
            fixture.schedule.certificate_binding_fingerprint_v2()
        );
        assert_ne!(
            foreign.graph_binding_fingerprint_v1(),
            fixture.schedule.graph_binding_fingerprint_v1()
        );
        let mut replay = boundary_configuration_replay_input_v2(
            fixture,
            policies,
            public_limits,
            authority,
            coverage_limits,
            endpoint_limits,
            schedule_limits,
            limits,
        );
        replay.schedule = &foreign;
        let mut polls = 0usize;
        assert_eq!(
            boundary.revalidate_with_checkpoint_v2(replay, || {
                polls += 1;
                Ok(())
            }),
            Err(CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
        );
        assert_eq!(polls, 1);
    }
}
