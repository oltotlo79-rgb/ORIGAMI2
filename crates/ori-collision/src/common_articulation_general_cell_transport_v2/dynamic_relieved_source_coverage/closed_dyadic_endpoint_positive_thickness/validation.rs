//! O(1) promotion validation, delegated replay, resources, and binding.

use std::mem::size_of;

use sha2::{Digest, Sha256};

use super::*;

pub(super) struct ValidatedEndpointCoverageV2 {
    pub(super) boundary_coverage: ClosedDyadicDomainBoundaryCoverageV2,
    pub(super) resources: EndpointCoverageResourcesV2,
    pub(super) binding_fingerprint: [u8; 32],
}

pub(super) fn validate_endpoint_coverage_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> Result<
    ValidatedEndpointCoverageV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    preflight_limits_v2(coverage, limits)?;
    let boundary_coverage = coverage
        .clearance
        .closed_dyadic_domain_boundary_coverage_seal_v2();
    if !boundary_coverage.is_complete_v2()
        || !coverage.all_source_order_pairs_covered_by_relieved_clearance_v2()
        || !coverage
            .clearance
            .whole_parent_positive_thickness_proven_v2()
    {
        return Err(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::BoundaryCoverageUnavailable,
        );
    }
    let resources = endpoint_resources_v2(coverage, limits)?;
    let binding_fingerprint = endpoint_binding_v2(coverage, boundary_coverage, resources, limits)?;
    Ok(ValidatedEndpointCoverageV2 {
        boundary_coverage,
        resources,
        binding_fingerprint,
    })
}

fn preflight_limits_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> Result<
    (),
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    if endpoint_limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || coverage.actual_block_count_v2() < GENERAL_N_MIN_BLOCKS_V2
        || coverage.actual_block_count_v2() > limits.max_blocks
        || coverage.limits.max_blocks > limits.max_blocks
        || size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>()
            > limits.max_retained_coverage_bytes
        || PROMOTION_LOGICAL_WORK_V2 > limits.max_promotion_logical_work
        || PROMOTION_WORKSPACE_BYTES_V2 > limits.max_promotion_workspace_bytes
        || size_of::<
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
        >() > limits.max_publication_bytes
    {
        return Err(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit,
        );
    }
    Ok(())
}

pub(super) fn preflight_replay_policy_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    input: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'_>,
) -> Result<
    (),
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    // At the Phase 3H boundary these nested policies are the exact identity
    // of the owned Phase 3G/3F proofs. Phase 3H's own resource envelope was
    // validated first; nested 0/MAX values are therefore identity mismatches,
    // not a reinterpretation of a foreign policy as an outer resource error.
    if !super::super::coverage_limits_match_v2(coverage.limits, input.limits)
        || !coverage.clearance.replay_limits_match_v2(input.live.limits)
    {
        return Err(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch,
        );
    }
    Ok(())
}

fn endpoint_resources_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> Result<
    EndpointCoverageResourcesV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    let retained_coverage_bytes =
        size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
    let publication_bytes = size_of::<
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    >();
    let delegated_replay_peak_bytes = retained_coverage_bytes
        .checked_add(coverage.limits.max_source_retained_bytes)
        .and_then(|value| value.checked_add(super::super::COVERAGE_WORKSPACE_BYTES_V2))
        .and_then(|value| {
            value.checked_add(coverage.clearance.replay_aggregate_peak_cap_v2())
        })
        .ok_or(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit,
        )?;
    let outer_publication_bytes = publication_bytes
        .checked_sub(retained_coverage_bytes)
        .ok_or(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit,
        )?;
    // Phase 3G's aggregate already includes its retained certificate shell.
    // Add only the Phase 3H shell delta and the fixed promotion workspace.
    let aggregate_peak_bytes = delegated_replay_peak_bytes
        .checked_add(outer_publication_bytes)
        .and_then(|value| value.checked_add(PROMOTION_WORKSPACE_BYTES_V2))
        .ok_or(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit,
        )?;
    let resources = EndpointCoverageResourcesV2 {
        retained_coverage_bytes,
        delegated_replay_peak_bytes,
        promotion_logical_work: PROMOTION_LOGICAL_WORK_V2,
        promotion_workspace_bytes: PROMOTION_WORKSPACE_BYTES_V2,
        publication_bytes,
        aggregate_peak_bytes,
    };
    if resources.retained_coverage_bytes > limits.max_retained_coverage_bytes
        || resources.promotion_logical_work > limits.max_promotion_logical_work
        || resources.promotion_workspace_bytes > limits.max_promotion_workspace_bytes
        || resources.publication_bytes > limits.max_publication_bytes
        || resources.aggregate_peak_bytes > limits.max_aggregate_peak_bytes
    {
        return Err(
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit,
        );
    }
    Ok(resources)
}

fn endpoint_binding_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    boundary: ClosedDyadicDomainBoundaryCoverageV2,
    resources: EndpointCoverageResourcesV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> Result<
    [u8; 32],
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_ENDPOINT_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
            .as_bytes(),
    );
    hash.update(b"canonical-normalized-closed-dyadic-domain-outer-boundaries");
    hash.update(coverage.binding_fingerprint);
    for value in [
        coverage.actual_block_count_v2(),
        coverage.material_face_count_v2(),
        coverage.source_order_pair_count_v2(),
        boundary.ordinary_lower_accepted_leaves,
        boundary.ordinary_upper_accepted_leaves,
        boundary.shared_relief_lower_accepted_leaves,
        boundary.shared_relief_upper_accepted_leaves,
        resources.retained_coverage_bytes,
        resources.delegated_replay_peak_bytes,
        resources.promotion_logical_work,
        resources.promotion_workspace_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    for value in endpoint_limit_values_v2(limits) {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

pub(super) fn revalidate_coverage_v2(
    coverage: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    input: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2,
    >,
) -> Result<
    (),
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    coverage
        .revalidate_with_checkpoint_v2(input, || {
            checkpoint().map_err(|stop| match stop {
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::Cancelled => {
                    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled
                }
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::DeadlineExceeded => {
                    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded
                }
            })
        })
        .map_err(|error| match error {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled => {
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Cancelled
            }
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded => {
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded
            }
            other => {
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Coverage(other)
            }
        })
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2,
    >,
) -> Result<
    (),
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::Cancelled => {
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Cancelled
        }
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded
        }
    })
}

pub(super) const fn endpoint_limits_match_v2(
    retained:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    live: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> bool {
    retained.max_blocks == live.max_blocks
        && retained.max_retained_coverage_bytes == live.max_retained_coverage_bytes
        && retained.max_promotion_logical_work == live.max_promotion_logical_work
        && retained.max_promotion_workspace_bytes == live.max_promotion_workspace_bytes
        && retained.max_publication_bytes == live.max_publication_bytes
        && retained.max_aggregate_peak_bytes == live.max_aggregate_peak_bytes
}

pub(super) const fn endpoint_limit_values_v2(
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
) -> [usize; 6] {
    [
        limits.max_blocks,
        limits.max_retained_coverage_bytes,
        limits.max_promotion_logical_work,
        limits.max_promotion_workspace_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}

fn update_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<
    (),
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| {
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::ResourceLimit
            })?
            .to_le_bytes(),
    );
    Ok(())
}
