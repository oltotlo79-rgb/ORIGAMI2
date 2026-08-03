use ori_foldability::GlobalFlatLayerOrderSourceAuthorityV2;
use ori_kinematics::CycleScheduleLimitsV1;

use super::super::*;
use super::support::{
    boundary_configuration_replay_input_v2, endpoint_replay_input_v2, limit_value_v2,
    replay_input_v2, set_boundary_configuration_limit_v2, set_endpoint_limit_v2, set_limit_v2,
};
use crate::CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_support::ReliefFixtureInputV2, support::OrdinaryFixtureV2,
};

pub(super) fn assert_preflight_limits_and_entry_stops_v2(
    certificate: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    fixture: &OrdinaryFixtureV2,
    policies: &ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    authority: &GlobalFlatLayerOrderSourceAuthorityV2<'_>,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) {
    for field in 0..12 {
        let exact = limit_value_v2(limits, field);
        for invalid in [0, usize::MAX] {
            assert_eq!(
                certificate.revalidate_v2(replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    set_limit_v2(limits, field, invalid),
                )),
                Err(
                    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit
                ),
                "limit {field} rejects {invalid}"
            );
        }
        let mut one_short_polls = 0usize;
        assert_eq!(
            certificate.revalidate_with_checkpoint_v2(
                replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    set_limit_v2(limits, field, exact - 1),
                ),
                || {
                    one_short_polls += 1;
                    Ok(())
                },
            ),
            Err(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit),
            "limit {field} rejects exact one-short"
        );
        assert_eq!(
            one_short_polls, 1,
            "limit {field} one-short fails before source replay"
        );
    }
    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled,
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded,
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            certificate.revalidate_with_checkpoint_v2(
                replay_input_v2(fixture, policies, public_limits, authority, limits),
                || Err(stop),
            ),
            Err(expected),
            "entry stop mapping"
        );
    }
}

pub(super) fn assert_endpoint_preflight_limits_and_entry_stops_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    fixture: &OrdinaryFixtureV2,
    policies: &ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    authority: &GlobalFlatLayerOrderSourceAuthorityV2<'_>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) {
    let values = [
        endpoint_limits.max_blocks,
        endpoint_limits.max_retained_coverage_bytes,
        endpoint_limits.max_promotion_logical_work,
        endpoint_limits.max_promotion_workspace_bytes,
        endpoint_limits.max_publication_bytes,
        endpoint_limits.max_aggregate_peak_bytes,
    ];
    for (field, exact) in values.into_iter().enumerate() {
        for invalid in [0, exact - 1, usize::MAX] {
            assert_eq!(
                endpoint.revalidate_v2(endpoint_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    coverage_limits,
                    set_endpoint_limit_v2(endpoint_limits, field, invalid),
                )),
                Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit),
                "endpoint limit {field} rejects {invalid}"
            );
        }
    }
    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::Cancelled,
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            endpoint.revalidate_with_checkpoint_v2(
                endpoint_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    coverage_limits,
                    endpoint_limits,
                ),
                || Err(stop),
            ),
            Err(expected),
            "endpoint entry stop mapping"
        );
    }

    // Nested policies are identities of the owned Phase 3G/3F proofs at this
    // outer boundary, even when a foreign tuple uses 0 or usize::MAX.
    for invalid in [0, usize::MAX] {
        let mut polls = 0usize;
        assert_eq!(
            endpoint.revalidate_with_checkpoint_v2(
                endpoint_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    set_limit_v2(coverage_limits, 1, invalid),
                    endpoint_limits,
                ),
                || {
                    polls += 1;
                    Ok(())
                },
            ),
            Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch),
            "nested Phase 3G policy rejects {invalid} as owned-proof identity"
        );
        assert_eq!(polls, 1);
    }
    for invalid in [0, usize::MAX] {
        let nested_clearance_limits = CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
            max_publication_bytes: invalid,
            ..public_limits
        };
        let mut polls = 0usize;
        assert_eq!(
            endpoint.revalidate_with_checkpoint_v2(
                endpoint_replay_input_v2(
                    fixture,
                    policies,
                    nested_clearance_limits,
                    authority,
                    coverage_limits,
                    endpoint_limits,
                ),
                || {
                    polls += 1;
                    Ok(())
                },
            ),
            Err(CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch),
            "nested Phase 3F policy rejects {invalid} as owned-proof identity"
        );
        assert_eq!(polls, 1);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assert_boundary_configuration_preflight_limits_and_entry_stops_v2(
    certificate: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    fixture: &OrdinaryFixtureV2,
    policies: &ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    authority: &GlobalFlatLayerOrderSourceAuthorityV2<'_>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    schedule_limits: CycleScheduleLimitsV1,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
) {
    let values = [
        limits.max_blocks,
        limits.max_hinges,
        limits.max_schedule_deep_retained_bytes,
        limits.max_boundary_evidence_logical_work,
        limits.max_boundary_evidence_workspace_bytes,
        limits.max_retained_endpoint_prerequisite_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ];
    let required = [
        certificate.actual_block_count_v2(),
        certificate.hinge_count_v2(),
        certificate.schedule_deep_retained_bytes_upper_bound_v2(),
        certificate.boundary_evidence_logical_work_v2(),
        certificate.boundary_evidence_workspace_bytes_upper_bound_v2(),
        certificate.retained_endpoint_prerequisite_bytes_v2(),
        certificate.publication_bytes_v2(),
        certificate.aggregate_peak_bytes_upper_bound_v2(),
    ];
    for (field, exact) in values.into_iter().enumerate() {
        for invalid in [0, exact - 1, usize::MAX] {
            let expected = if invalid == 0 || invalid == usize::MAX || invalid < required[field] {
                CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::ResourceLimit
            } else {
                CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch
            };
            assert_eq!(
                certificate.revalidate_v2(boundary_configuration_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    set_boundary_configuration_limit_v2(limits, field, invalid),
                )),
                Err(expected),
                "boundary-configuration limit {field} rejects {invalid}"
            );
        }
    }
    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2::Cancelled,
            CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
            CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            certificate.revalidate_with_checkpoint_v2(
                boundary_configuration_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    limits,
                ),
                || Err(stop),
            ),
            Err(expected),
            "boundary-configuration entry stop mapping"
        );
    }
}
