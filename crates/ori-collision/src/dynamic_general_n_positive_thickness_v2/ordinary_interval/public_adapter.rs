//! Crate-private adapter from the public direct facade to Phase 3E.

use std::mem::size_of;

use super::relief_aggregate::{
    ReliefAggregateErrorV2, ReliefAggregateInputV2, ReliefAggregateLimitsV2,
    WholeParentPositiveThicknessAdapterSealV2, into_public_adapter_seal_v2,
    prove_whole_parent_positive_thickness_with_checkpoint_v2,
};
use super::*;
use crate::common_articulation_dynamic_general_n_relieved_clearance_v2::{
    CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
    CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
    CommonArticulationDynamicGeneralNRelievedClearanceInputV2,
    CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
};

#[path = "public_adapter/binding.rs"]
mod adapter_binding;

const ADAPTER_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_relieved_clearance_adapter_v2";
const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const HARD_MAX_UNORDERED_FACE_PAIRS_V2: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdapterStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdapterErrorV2 {
    InvalidInput,
    ResourceLimit,
    UnsupportedSharedTopology,
    UnprovenSharedRelief,
    OrdinaryProofUnavailable,
    Cancelled,
    DeadlineExceeded,
}

/// Opaque backing retained only by the public certificate facade.
pub(crate) struct DirectClearanceEvidenceV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    adapter_binding: [u8; 32],
    actual_block_count: usize,
    total_face_pairs: usize,
    ordinary_face_pairs: usize,
    shared_hinge_pairs: usize,
    shared_vertex_pairs: usize,
    aggregate_peak_bytes: usize,
}

impl std::fmt::Debug for DirectClearanceEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DirectClearanceEvidenceV2")
            .field("model", &ADAPTER_MODEL_ID_V2)
            .field("actual_block_count", &self.actual_block_count)
            .field("total_face_pairs", &self.total_face_pairs)
            .finish_non_exhaustive()
    }
}

impl DirectClearanceEvidenceV2 {
    pub(crate) fn matches_v2(&self, candidate: &Self) -> bool {
        self.issuer_geometry == candidate.issuer_geometry
            && self.adapter_binding == candidate.adapter_binding
            && self.actual_block_count == candidate.actual_block_count
            && self.total_face_pairs == candidate.total_face_pairs
            && self.ordinary_face_pairs == candidate.ordinary_face_pairs
            && self.shared_hinge_pairs == candidate.shared_hinge_pairs
            && self.shared_vertex_pairs == candidate.shared_vertex_pairs
            && self.aggregate_peak_bytes == candidate.aggregate_peak_bytes
    }

    pub(crate) const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    pub(crate) const fn total_face_pairs_v2(&self) -> usize {
        self.total_face_pairs
    }

    pub(crate) const fn ordinary_face_pairs_v2(&self) -> usize {
        self.ordinary_face_pairs
    }

    pub(crate) const fn shared_hinge_pairs_v2(&self) -> usize {
        self.shared_hinge_pairs
    }

    pub(crate) const fn shared_vertex_pairs_v2(&self) -> usize {
        self.shared_vertex_pairs
    }

    pub(crate) const fn aggregate_peak_bytes_v2(&self) -> usize {
        self.aggregate_peak_bytes
    }
}

pub(crate) fn prove_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), AdapterStopV2>,
) -> Result<DirectClearanceEvidenceV2, AdapterErrorV2> {
    adapter_checkpoint_v2(&mut checkpoint)?;
    let preflight = preflight_adapter_v2(&input)?;
    let (shared_pair_registry_bytes, seal) = {
        let mut private_checkpoint = || checkpoint().map_err(map_adapter_stop_v2);
        let shared_pairs = super::geometry::derive_exact_shared_pair_registry_v2(
            input.geometry,
            input.limits.ordinary.max_excluded_shared_pairs,
            input.limits.ordinary.max_shared_feature_membership_tests,
            &mut private_checkpoint,
        )
        .map_err(map_ordinary_error_v2)?;
        let shared_pair_registry_bytes = shared_pairs
            .capacity()
            .checked_mul(size_of::<OrdinaryIntervalFacePairV2>())
            .ok_or(AdapterErrorV2::ResourceLimit)?;
        if shared_pairs.capacity() > input.limits.ordinary.max_excluded_shared_pairs
            || shared_pair_registry_bytes > input.limits.max_shared_pair_registry_bytes
        {
            return Err(AdapterErrorV2::ResourceLimit);
        }
        super::checkpoint_v2(&mut private_checkpoint).map_err(map_ordinary_error_v2)?;
        let ordinary = OrdinaryIntervalInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            pose: input.pose,
            fixed_face: input.parent_fixed_face,
            schedule: input.parent_schedule,
            decomposition: input.decomposition,
            common_pose: input.common_pose,
            profile: input.profile,
            dynamic_closure_bridge: input.dynamic_closure_bridge,
            paper_thickness_mm: input.paper_thickness_mm,
            closure_tolerance: input.closure_tolerance,
            excluded_shared_pairs: shared_pairs.as_slice(),
            limits: adapter_binding::ordinary_limits_v2(input.limits.ordinary),
        };
        let evidence = prove_whole_parent_positive_thickness_with_checkpoint_v2(
            ReliefAggregateInputV2 {
                ordinary,
                hinge_policies: input.hinge_policies,
                vertex_policies: input.vertex_policies,
                limits: adapter_binding::relief_limits_v2(input.limits.relief),
            },
            &mut private_checkpoint,
        )
        .map_err(map_relief_error_v2)?;
        (
            shared_pair_registry_bytes,
            into_public_adapter_seal_v2(evidence),
        )
    };
    adapter_checkpoint_v2(&mut checkpoint)?;
    finish_adapter_v2(
        &input,
        preflight,
        shared_pair_registry_bytes,
        seal,
        &mut checkpoint,
    )
}

#[derive(Clone, Copy)]
struct AdapterPreflightV2 {
    actual_block_count: usize,
    publication_bytes: usize,
}

fn preflight_adapter_v2(
    input: &CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
) -> Result<AdapterPreflightV2, AdapterErrorV2> {
    let limits = input.limits;
    if [
        limits.max_blocks,
        limits.max_shared_pair_registry_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
        limits.ordinary.max_excluded_shared_pairs,
        limits.ordinary.max_shared_feature_membership_tests,
        limits.relief.max_shared_pairs,
        limits.relief.max_aggregate_peak_bytes,
    ]
    .into_iter()
    .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(AdapterErrorV2::ResourceLimit);
    }
    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || actual_block_count > limits.max_blocks
        || limits.max_blocks > configured_max_blocks
        || actual.block_count_v2() != actual_block_count
        || actual_block_count > maximum.block_count_v2()
        || input.geometry.face_ids().len() != actual.face_count_v2()
        || input.geometry.hinges().len() != actual.hinge_count_v2()
    {
        return Err(AdapterErrorV2::ResourceLimit);
    }
    let total_face_pairs = checked_unordered_pairs_v2(input.geometry.face_ids().len())?;
    if total_face_pairs > HARD_MAX_UNORDERED_FACE_PAIRS_V2 {
        return Err(AdapterErrorV2::ResourceLimit);
    }
    let pair_capacity = total_face_pairs.min(limits.ordinary.max_excluded_shared_pairs);
    let declared_registry_bytes = pair_capacity
        .checked_mul(size_of::<OrdinaryIntervalFacePairV2>())
        .ok_or(AdapterErrorV2::ResourceLimit)?;
    let publication_bytes = size_of::<DirectClearanceEvidenceV2>();
    let declared_peak = declared_registry_bytes
        .checked_add(limits.relief.max_aggregate_peak_bytes)
        .and_then(|value| value.checked_add(publication_bytes))
        .ok_or(AdapterErrorV2::ResourceLimit)?;
    if declared_registry_bytes > limits.max_shared_pair_registry_bytes
        || publication_bytes > limits.max_publication_bytes
        || declared_peak > limits.max_aggregate_peak_bytes
    {
        return Err(AdapterErrorV2::ResourceLimit);
    }
    Ok(AdapterPreflightV2 {
        actual_block_count,
        publication_bytes,
    })
}

fn finish_adapter_v2(
    input: &CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    preflight: AdapterPreflightV2,
    shared_pair_registry_bytes: usize,
    seal: WholeParentPositiveThicknessAdapterSealV2,
    checkpoint: &mut impl FnMut() -> Result<(), AdapterStopV2>,
) -> Result<DirectClearanceEvidenceV2, AdapterErrorV2> {
    if !seal.issuer_geometry.matches(input.geometry) {
        return Err(AdapterErrorV2::InvalidInput);
    }
    let aggregate_peak_bytes = shared_pair_registry_bytes
        .checked_add(seal.aggregate_peak_bytes)
        .and_then(|value| value.checked_add(preflight.publication_bytes))
        .ok_or(AdapterErrorV2::ResourceLimit)?;
    if aggregate_peak_bytes > input.limits.max_aggregate_peak_bytes {
        return Err(AdapterErrorV2::ResourceLimit);
    }
    let adapter_binding = adapter_binding::adapter_binding_v2(
        input,
        &seal,
        preflight.actual_block_count,
        shared_pair_registry_bytes,
        aggregate_peak_bytes,
    )?;
    adapter_checkpoint_v2(checkpoint)?;
    Ok(DirectClearanceEvidenceV2 {
        issuer_geometry: seal.issuer_geometry,
        adapter_binding,
        actual_block_count: preflight.actual_block_count,
        total_face_pairs: seal.total_face_pairs,
        ordinary_face_pairs: seal.ordinary_pairs,
        shared_hinge_pairs: seal.shared_hinge_pairs,
        shared_vertex_pairs: seal.shared_vertex_pairs,
        aggregate_peak_bytes,
    })
}

fn checked_unordered_pairs_v2(face_count: usize) -> Result<usize, AdapterErrorV2> {
    face_count
        .checked_mul(face_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(AdapterErrorV2::ResourceLimit)
}

fn adapter_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), AdapterStopV2>,
) -> Result<(), AdapterErrorV2> {
    checkpoint().map_err(|stop| match stop {
        AdapterStopV2::Cancelled => AdapterErrorV2::Cancelled,
        AdapterStopV2::DeadlineExceeded => AdapterErrorV2::DeadlineExceeded,
    })
}

const fn map_adapter_stop_v2(stop: AdapterStopV2) -> OrdinaryIntervalStopV2 {
    match stop {
        AdapterStopV2::Cancelled => OrdinaryIntervalStopV2::Cancelled,
        AdapterStopV2::DeadlineExceeded => OrdinaryIntervalStopV2::DeadlineExceeded,
    }
}

const fn map_ordinary_error_v2(error: OrdinaryIntervalErrorV2) -> AdapterErrorV2 {
    match error {
        OrdinaryIntervalErrorV2::ResourceLimit => AdapterErrorV2::ResourceLimit,
        OrdinaryIntervalErrorV2::UnprovenOrdinaryClearance => {
            AdapterErrorV2::OrdinaryProofUnavailable
        }
        OrdinaryIntervalErrorV2::Cancelled => AdapterErrorV2::Cancelled,
        OrdinaryIntervalErrorV2::DeadlineExceeded => AdapterErrorV2::DeadlineExceeded,
        OrdinaryIntervalErrorV2::InvalidInput
        | OrdinaryIntervalErrorV2::NonCanonicalExcludedSharedPairRegistry
        | OrdinaryIntervalErrorV2::DuplicateExcludedSharedPair
        | OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch => {
            AdapterErrorV2::InvalidInput
        }
    }
}

const fn map_relief_error_v2(error: ReliefAggregateErrorV2) -> AdapterErrorV2 {
    match error {
        ReliefAggregateErrorV2::InvalidInput => AdapterErrorV2::InvalidInput,
        ReliefAggregateErrorV2::ResourceLimit => AdapterErrorV2::ResourceLimit,
        ReliefAggregateErrorV2::UnsupportedSharedTopology => {
            AdapterErrorV2::UnsupportedSharedTopology
        }
        ReliefAggregateErrorV2::UnprovenSharedRelief => AdapterErrorV2::UnprovenSharedRelief,
        ReliefAggregateErrorV2::OrdinaryProofUnavailable => {
            AdapterErrorV2::OrdinaryProofUnavailable
        }
        ReliefAggregateErrorV2::Cancelled => AdapterErrorV2::Cancelled,
        ReliefAggregateErrorV2::DeadlineExceeded => AdapterErrorV2::DeadlineExceeded,
    }
}
