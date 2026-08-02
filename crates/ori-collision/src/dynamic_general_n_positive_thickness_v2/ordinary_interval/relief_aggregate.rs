//! Sealed continuous relief for the shared pairs excluded by the ordinary
//! interval kernel, plus an exhaustive whole-parent aggregate.

use crate::{HingeReliefPolicyRecordV1, VertexReliefPolicyRecordV1};
use ori_domain::{EdgeId, VertexId};
use ori_kinematics::{
    CommonArticulationDynamicClosureBridgeStopV2,
    CommonArticulationDynamicClosureIntervalTransformLeafErrorV2,
    CommonArticulationDynamicClosureIntervalTransformLeafV2, MaterialHingeGraphInstanceV1,
    OutwardIntervalErrorV1, OutwardIntervalV1,
};
use sha2::{Digest, Sha256};

use super::*;

mod binding;
mod classification;
mod exact_clip;
mod geometry;
mod resources;

const RELIEF_MODEL_ID_V2: &str = "common_articulation_dynamic_general_n_shared_relief_v2";
const AGGREGATE_MODEL_ID_V2: &str = "common_articulation_dynamic_general_n_positive_thickness_v2";
const HARD_MAX_RELIEF_RECORDS_V2: usize = 4_096;
const HARD_MAX_SHARED_PAIRS_V2: usize = 1 << 20;
const HARD_MAX_RELIEF_LEAVES_V2: usize = 1 << 16;
const HARD_MAX_SQRT_OPERATIONS_V2: usize = 20_000;
const HARD_MAX_EXACT_VALUE_BITS_V2: usize = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReliefAggregateErrorV2 {
    InvalidInput,
    ResourceLimit,
    UnsupportedSharedTopology,
    UnprovenSharedRelief,
    OrdinaryProofUnavailable,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReliefAggregateLimitsV2 {
    pub max_hinge_policy_records: usize,
    pub max_vertex_policy_records: usize,
    pub max_vertex_incident_face_occurrences: usize,
    pub max_shared_pairs: usize,
    pub max_pair_membership_tests: usize,
    pub max_pair_hinge_tests: usize,
    /// Covers the two whole-parent canonical-quad scope scans plus all live
    /// policy-registry validation and its bounded use-ledger bookkeeping.
    pub max_scope_and_policy_validation_work: usize,
    pub max_convexity_segment_tests: usize,
    pub max_rest_carrier_vertices: usize,
    pub max_exact_clip_operations: usize,
    pub max_sqrt_calls: usize,
    pub max_sqrt_operations_per_call: usize,
    pub max_exact_value_bits: usize,
    pub max_exact_scratch_bytes: usize,
    pub max_collision_depth: u32,
    pub max_collision_leaves: usize,
    pub max_shared_pair_node_tests: usize,
    pub max_axis_projection_work: usize,
    pub max_carrier_conversion_work: usize,
    pub max_hash_work: usize,
    /// Sum of the ten variable-cost counters reported below. Small structural
    /// registry lookups, fixed-quad endpoint checks, retained-capacity ledger
    /// scans, and fixed-field binding hashes remain separately finite under
    /// the submitted cardinality caps.
    pub max_logical_work: usize,
    pub max_temporary_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ReliefAggregateResourcesV2 {
    pub hinge_policy_records: usize,
    pub vertex_policy_records: usize,
    pub vertex_incident_face_occurrences: usize,
    pub shared_pairs: usize,
    pub shared_hinge_pairs: usize,
    pub shared_vertex_pairs: usize,
    pub pair_membership_tests: usize,
    pub pair_hinge_tests: usize,
    /// Conservatively charges both the outer resource preflight and the
    /// post-ordinary relief revalidation, plus all policy/use-ledger
    /// validation. Private test entry points preserve the same two-pass
    /// contract.
    pub scope_and_policy_validation_work: usize,
    pub convexity_segment_tests: usize,
    pub rest_carrier_vertices: usize,
    /// Conservatively charged structural upper bound; observed work is
    /// checked not to exceed it before publication.
    pub exact_clip_operations: usize,
    pub sqrt_calls: usize,
    pub processed_interval_nodes: usize,
    pub accepted_interval_leaves: usize,
    pub certified_shared_pair_leaf_count: usize,
    /// Full `max_collision_leaves * 2 - 1` structural charge.
    pub shared_pair_node_tests: usize,
    /// Full all-carrier/all-node projection charge.
    pub axis_projection_work: usize,
    pub carrier_conversion_work: usize,
    /// Full variable-length registry and partition hash charge. Fixed-size
    /// model/binding hash fields are constant structural work.
    pub hash_work: usize,
    /// Sum of membership, hinge, scope/policy, convexity, exact-clip,
    /// pair-node, projection, conversion, hash, and bounded-sqrt work.
    pub logical_work: usize,
    pub retained_carrier_bytes: usize,
    pub exact_scratch_bytes: usize,
    pub temporary_bytes: usize,
    pub publication_bytes: usize,
    pub aggregate_peak_bytes: usize,
}

#[derive(Clone, Copy)]
pub(super) struct ReliefAggregateInputV2<'a> {
    pub ordinary: OrdinaryIntervalInputV2<'a>,
    pub hinge_policies: &'a [HingeReliefPolicyRecordV1],
    pub vertex_policies: &'a [VertexReliefPolicyRecordV1],
    pub limits: ReliefAggregateLimitsV2,
}

/// Private proof material only.  It intentionally has no Clone, serde,
/// accessor, conversion, or authorization surface.
pub(super) struct WholeParentPositiveThicknessEvidenceV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    ordinary_binding: [u8; 32],
    relief_binding: [u8; 32],
    aggregate_binding: [u8; 32],
    shared_pair_digest: [u8; 32],
    total_face_pairs: usize,
    ordinary_pairs: usize,
    shared_hinge_pairs: usize,
    shared_vertex_pairs: usize,
    resources: ReliefAggregateResourcesV2,
    limits: ReliefAggregateLimitsV2,
}

/// Narrow one-way seal consumed by the crate-private public adapter. It omits
/// the ordinary/relief evidence and exposes no public construction boundary.
pub(super) struct WholeParentPositiveThicknessAdapterSealV2 {
    pub(super) issuer_geometry: MaterialHingeGraphInstanceV1,
    pub(super) aggregate_binding: [u8; 32],
    pub(super) total_face_pairs: usize,
    pub(super) ordinary_pairs: usize,
    pub(super) shared_hinge_pairs: usize,
    pub(super) shared_vertex_pairs: usize,
    pub(super) aggregate_peak_bytes: usize,
}

struct SharedReliefEvidenceV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    shared_pair_digest: [u8; 32],
    policy_digest: [u8; 32],
    partition_digest: [u8; 32],
    binding: [u8; 32],
    resources: ReliefAggregateResourcesV2,
    limits: ReliefAggregateLimitsV2,
}

impl std::fmt::Debug for WholeParentPositiveThicknessEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WholeParentPositiveThicknessEvidenceV2")
            .field("model", &AGGREGATE_MODEL_ID_V2)
            .field("total_face_pairs", &self.total_face_pairs)
            .field("ordinary_pairs", &self.ordinary_pairs)
            .field("shared_hinge_pairs", &self.shared_hinge_pairs)
            .field("shared_vertex_pairs", &self.shared_vertex_pairs)
            .field("resources", &self.resources)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for SharedReliefEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedReliefEvidenceV2")
            .field("model", &RELIEF_MODEL_ID_V2)
            .field("resources", &self.resources)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedFeatureV2 {
    Hinge { edge: EdgeId },
    Vertex { vertex: VertexId },
}

#[derive(Debug)]
struct PreparedCellV2 {
    face: FaceId,
    anchor: [f64; 3],
    support_axis: [f64; 3],
    ring: Vec<[OutwardIntervalV1; 2]>,
}

#[derive(Debug)]
struct PreparedSharedPairV2 {
    pair: OrdinaryIntervalFacePairV2,
    feature: SharedFeatureV2,
    left: PreparedCellV2,
    right: PreparedCellV2,
}

struct ValidatedReliefV2<'a> {
    ordinary: ValidatedInputV2<'a>,
    pairs: Vec<PreparedSharedPairV2>,
    shared_pair_digest: [u8; 32],
    policy_digest: [u8; 32],
    resources: ReliefAggregateResourcesV2,
}

pub(super) fn prove_whole_parent_positive_thickness_v2(
    input: ReliefAggregateInputV2<'_>,
) -> Result<WholeParentPositiveThicknessEvidenceV2, ReliefAggregateErrorV2> {
    prove_whole_parent_positive_thickness_with_checkpoint_v2(input, || Ok(()))
}

pub(super) fn into_public_adapter_seal_v2(
    evidence: WholeParentPositiveThicknessEvidenceV2,
) -> WholeParentPositiveThicknessAdapterSealV2 {
    WholeParentPositiveThicknessAdapterSealV2 {
        issuer_geometry: evidence.issuer_geometry,
        aggregate_binding: evidence.aggregate_binding,
        total_face_pairs: evidence.total_face_pairs,
        ordinary_pairs: evidence.ordinary_pairs,
        shared_hinge_pairs: evidence.shared_hinge_pairs,
        shared_vertex_pairs: evidence.shared_vertex_pairs,
        aggregate_peak_bytes: evidence.resources.aggregate_peak_bytes,
    }
}

#[cfg(test)]
pub(super) fn inspect_whole_parent_evidence_for_test_v2(
    evidence: &WholeParentPositiveThicknessEvidenceV2,
) -> (usize, usize, usize, usize, ReliefAggregateResourcesV2) {
    (
        evidence.total_face_pairs,
        evidence.ordinary_pairs,
        evidence.shared_hinge_pairs,
        evidence.shared_vertex_pairs,
        evidence.resources,
    )
}

#[cfg(test)]
pub(super) fn prove_shared_relief_for_test_v2(
    input: ReliefAggregateInputV2<'_>,
) -> Result<ReliefAggregateResourcesV2, ReliefAggregateErrorV2> {
    prove_shared_relief_with_checkpoint_for_test_v2(input, || Ok(()))
}

#[cfg(test)]
pub(super) fn prove_shared_relief_with_checkpoint_for_test_v2(
    input: ReliefAggregateInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<ReliefAggregateResourcesV2, ReliefAggregateErrorV2> {
    relief_checkpoint_v2(&mut checkpoint)?;
    resources::preflight_limits_v2(&input, &mut checkpoint)?;
    let ordinary_preflight =
        super::resources::preflight_input_resources_v2(&input.ordinary, &mut checkpoint)
            .map_err(map_ordinary_error_v2)?;
    resources::preflight_observed_ordinary_v2(&input, ordinary_preflight.resources)?;
    let ordinary_resources = ordinary_preflight.resources;
    drop(ordinary_preflight);
    prove_shared_relief_with_checkpoint_v2(input, ordinary_resources, &mut checkpoint)
        .map(|evidence| evidence.resources)
}

#[cfg(test)]
pub(super) fn finite_hinge_axial_position_for_test_v2(axial: i64, hinge_axis_squared: i64) -> bool {
    exact_clip::finite_hinge_axial_position_for_test_v2(axial, hinge_axis_squared)
}

#[cfg(test)]
pub(super) fn strict_intervals_disjoint_for_test_v2(left: [f64; 2], right: [f64; 2]) -> bool {
    geometry::strict_intervals_disjoint_for_test_v2(left, right)
}

#[cfg(test)]
pub(super) fn validate_hinge_policy_for_test_v2(
    input: &ReliefAggregateInputV2<'_>,
    policy: &HingeReliefPolicyRecordV1,
) -> Result<usize, ReliefAggregateErrorV2> {
    let mut resources = ReliefAggregateResourcesV2::default();
    exact_clip::validate_hinge_policy_v2(policy, input, &mut resources)?;
    Ok(resources.exact_clip_operations)
}

#[cfg(test)]
pub(super) fn validate_vertex_policy_for_test_v2(
    input: &ReliefAggregateInputV2<'_>,
    policy: &VertexReliefPolicyRecordV1,
) -> Result<usize, ReliefAggregateErrorV2> {
    let mut resources = ReliefAggregateResourcesV2::default();
    exact_clip::validate_vertex_policy_v2(policy, input, &mut resources)?;
    Ok(resources.exact_clip_operations)
}

#[cfg(test)]
pub(super) fn preflight_whole_parent_for_test_v2(
    input: ReliefAggregateInputV2<'_>,
) -> Result<(), ReliefAggregateErrorV2> {
    let mut no_stop = || Ok(());
    resources::preflight_limits_v2(&input, &mut no_stop)?;
    let ordinary = super::resources::preflight_input_resources_v2(&input.ordinary, &mut no_stop)
        .map_err(map_ordinary_error_v2)?;
    resources::preflight_observed_ordinary_v2(&input, ordinary.resources)
}

pub(super) fn prove_whole_parent_positive_thickness_with_checkpoint_v2(
    input: ReliefAggregateInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<WholeParentPositiveThicknessEvidenceV2, ReliefAggregateErrorV2> {
    relief_checkpoint_v2(&mut checkpoint)?;
    resources::preflight_limits_v2(&input, &mut checkpoint)?;
    // Resource-only live replay establishes the ordinary session's observed
    // peak before either the interval-transform session or the expensive
    // ordinary collision partition is allocated. The later ordinary and
    // relief sessions are sequential and never coexist.
    let ordinary_preflight =
        super::resources::preflight_input_resources_v2(&input.ordinary, &mut checkpoint)
            .map_err(map_ordinary_error_v2)?;
    resources::preflight_observed_ordinary_v2(&input, ordinary_preflight.resources)?;
    drop(ordinary_preflight);
    relief_checkpoint_v2(&mut checkpoint)?;
    let ordinary =
        prove_ordinary_interval_clearance_with_checkpoint_v2(input.ordinary, &mut checkpoint)
            .map_err(map_ordinary_error_v2)?;
    resources::preflight_observed_ordinary_v2(&input, ordinary.resources)?;
    relief_checkpoint_v2(&mut checkpoint)?;
    let relief =
        prove_shared_relief_with_checkpoint_v2(input, ordinary.resources, &mut checkpoint)?;
    relief_checkpoint_v2(&mut checkpoint)?;
    if ordinary.excluded_shared_pair_digest != relief.shared_pair_digest
        || !ordinary.issuer_geometry.matches(input.ordinary.geometry)
        || !relief.issuer_geometry.matches(input.ordinary.geometry)
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    let total_face_pairs = ordinary
        .resources
        .ordinary_face_pairs
        .checked_add(relief.resources.shared_pairs)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    if total_face_pairs != ordinary.resources.total_face_pairs
        || relief.resources.shared_pairs
            != relief
                .resources
                .shared_hinge_pairs
                .checked_add(relief.resources.shared_vertex_pairs)
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    let aggregate_binding = binding::aggregate_binding_v2(&input, &ordinary, &relief)?;
    relief_checkpoint_v2(&mut checkpoint)?;
    Ok(WholeParentPositiveThicknessEvidenceV2 {
        issuer_geometry: input.ordinary.geometry.instance_anchor_v1(),
        ordinary_binding: ordinary.binding_fingerprint,
        relief_binding: relief.binding,
        aggregate_binding,
        shared_pair_digest: relief.shared_pair_digest,
        total_face_pairs,
        ordinary_pairs: ordinary.resources.ordinary_face_pairs,
        shared_hinge_pairs: relief.resources.shared_hinge_pairs,
        shared_vertex_pairs: relief.resources.shared_vertex_pairs,
        resources: relief.resources,
        limits: input.limits,
    })
}

fn prove_shared_relief_with_checkpoint_v2(
    input: ReliefAggregateInputV2<'_>,
    ordinary_resources: OrdinaryIntervalResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<SharedReliefEvidenceV2, ReliefAggregateErrorV2> {
    let mut validated = classification::validate_relief_input_v2(&input, checkpoint)?;
    let partition_digest = geometry::prove_relief_partition_v2(&input, &mut validated, checkpoint)?;
    relief_checkpoint_v2(checkpoint)?;
    resources::finish_resource_accounting_v2(&input, ordinary_resources, &mut validated.resources)?;
    let binding = binding::relief_binding_v2(&input, &validated, partition_digest)?;
    Ok(SharedReliefEvidenceV2 {
        issuer_geometry: input.ordinary.geometry.instance_anchor_v1(),
        shared_pair_digest: validated.shared_pair_digest,
        policy_digest: validated.policy_digest,
        partition_digest,
        binding,
        resources: validated.resources,
        limits: input.limits,
    })
}

fn relief_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), ReliefAggregateErrorV2> {
    checkpoint().map_err(|stop| match stop {
        OrdinaryIntervalStopV2::Cancelled => ReliefAggregateErrorV2::Cancelled,
        OrdinaryIntervalStopV2::DeadlineExceeded => ReliefAggregateErrorV2::DeadlineExceeded,
    })
}

fn map_ordinary_error_v2(error: OrdinaryIntervalErrorV2) -> ReliefAggregateErrorV2 {
    match error {
        OrdinaryIntervalErrorV2::Cancelled => ReliefAggregateErrorV2::Cancelled,
        OrdinaryIntervalErrorV2::DeadlineExceeded => ReliefAggregateErrorV2::DeadlineExceeded,
        OrdinaryIntervalErrorV2::ResourceLimit => ReliefAggregateErrorV2::ResourceLimit,
        OrdinaryIntervalErrorV2::UnprovenOrdinaryClearance => {
            ReliefAggregateErrorV2::OrdinaryProofUnavailable
        }
        OrdinaryIntervalErrorV2::InvalidInput
        | OrdinaryIntervalErrorV2::NonCanonicalExcludedSharedPairRegistry
        | OrdinaryIntervalErrorV2::DuplicateExcludedSharedPair
        | OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch => {
            ReliefAggregateErrorV2::InvalidInput
        }
    }
}

fn map_interval_error_v2(error: OutwardIntervalErrorV1) -> ReliefAggregateErrorV2 {
    match error {
        OutwardIntervalErrorV1::ResourceLimit => ReliefAggregateErrorV2::ResourceLimit,
        OutwardIntervalErrorV1::InvalidEndpoint
        | OutwardIntervalErrorV1::DivisionByZeroInterval => {
            ReliefAggregateErrorV2::UnprovenSharedRelief
        }
    }
}
