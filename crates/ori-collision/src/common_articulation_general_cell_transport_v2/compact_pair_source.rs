//! No-search compact-pair source bridge for the General-N transport replay.
//!
//! The bridge never treats packed direction bits as a transport authority.
//! It first revalidates the compact authority's complete retained layer-order
//! snapshot against live foldability inputs, then delegates all geometry,
//! clearance, and source-shape checks to the ordinary V2 transport boundary.

use std::mem::size_of;

use ori_foldability::{
    GLOBAL_FLAT_LAYER_ORDER_COMPACT_PAIR_ASSIGNMENT_DOMAIN_V2, GlobalFlatFoldabilityCheckpoint,
    GlobalFlatFoldabilityExecutionError, GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits,
    GlobalFlatFoldabilityObserver, GlobalFlatFoldabilityProvenance,
    GlobalFlatFoldabilityUnknownReason, GlobalFlatFoldabilityWorkCounts,
    GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
    GlobalFlatLayerOrderCompactPairAssignmentResourcesV2, GlobalFlatLayerOrderRevalidationErrorV2,
    GlobalFlatLayerOrderRevalidationLimitsV2, GlobalFlatLayerOrderSourceAuthorityV2,
    LayerOrderSnapshot, revalidate_global_flat_layer_order_source_with_observer_v2,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::*;

/// Stable domain identifier for the compact-pair General-N observation.
pub const COMMON_ARTICULATION_COMPACT_PAIR_GENERAL_CELL_TRANSPORT_MODEL_ID_V2: &str =
    "common_articulation_compact_pair_general_cell_transport_v2";

const COMPACT_PAIR_TRANSPORT_BASE_WORK_V2: usize = 64;
const COMPACT_PAIR_TRANSPORT_WORKSPACE_BYTES_V2: usize = 512;
const DIRECTION_HASH_CHUNK_BYTES_V2: usize = 4 * 1024;

/// Fail-closed error from compact-source admission or replay.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommonArticulationCompactPairGeneralCellTransportErrorV2 {
    #[error("compact-pair transport limits must all be finite")]
    InvalidLimits,
    #[error("the compact direction assignment is malformed")]
    MalformedAssignment,
    #[error("the compact-pair transport input exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the supplied authority does not authenticate a no-search compact assignment")]
    NoSearchInvariant,
    #[error("the compact pair assignment does not match its opaque authority")]
    CompactSourceBindingMismatch,
    #[error("the compact source does not revalidate against the live foldability input: {0}")]
    SourceRevalidation(GlobalFlatLayerOrderRevalidationErrorV2),
    #[error("the delegated General-N transport replay failed: {0}")]
    Transport(CommonArticulationGeneralCellTransportErrorV2),
    #[error("the retained compact-pair transport observation does not match the live input")]
    PrerequisiteBindingMismatch,
    #[error("the compact-pair transport operation was cancelled")]
    Cancelled,
    #[error("the compact-pair transport operation deadline elapsed")]
    DeadlineExceeded,
}

/// Explicit finite caps owned by the compact-to-transport bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationCompactPairGeneralCellTransportLimitsV2 {
    pub max_compact_assignment_bytes: usize,
    pub max_compact_source_retained_bytes: usize,
    pub max_logical_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

/// Deterministic bridge charges retained with the unpromoted observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationCompactPairGeneralCellTransportResourcesV2 {
    pub compact_assignment_bytes: usize,
    pub compact_source_retained_bytes: usize,
    pub source_revalidation_peak_bytes: usize,
    pub transport_logical_work: usize,
    pub transport_retained_bytes: usize,
    pub transport_peak_bytes: usize,
    pub logical_work: usize,
    pub retained_bytes: usize,
    pub peak_bytes: usize,
}

/// The non-source half of one General-N transport issue or replay.
#[derive(Clone, Copy)]
pub struct CommonArticulationCompactPairGeneralCellTransportLiveInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub whole_parent_closure: &'a CommonArticulationWholeParentClosureV2,
    pub whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    pub clearance: &'a CommonArticulationClearancePrerequisiteV2,
    pub transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
}

impl<'a> CommonArticulationCompactPairGeneralCellTransportLiveInputV2<'a> {
    fn issue_input(
        self,
        source_authority: GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    ) -> CommonArticulationGeneralCellTransportInputV2<'a> {
        CommonArticulationGeneralCellTransportInputV2 {
            geometry: self.geometry,
            audit: self.audit,
            pose: self.pose,
            decomposition: self.decomposition,
            common_pose: self.common_pose,
            parent_fixed_face: self.parent_fixed_face,
            parent_schedule: self.parent_schedule,
            profile: self.profile,
            paper_thickness_mm: self.paper_thickness_mm,
            closure_tolerance: self.closure_tolerance,
            block_closure_set: self.block_closure_set,
            whole_parent_closure: self.whole_parent_closure,
            whole_parent_closure_limits: self.whole_parent_closure_limits,
            clearance: self.clearance,
            source_authority,
            limits: self.transport_limits,
        }
    }

    fn revalidation_input(
        self,
        source_authority: GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    ) -> CommonArticulationGeneralCellTransportRevalidationInputV2<'a> {
        CommonArticulationGeneralCellTransportRevalidationInputV2 {
            geometry: self.geometry,
            audit: self.audit,
            pose: self.pose,
            decomposition: self.decomposition,
            common_pose: self.common_pose,
            parent_fixed_face: self.parent_fixed_face,
            parent_schedule: self.parent_schedule,
            profile: self.profile,
            paper_thickness_mm: self.paper_thickness_mm,
            closure_tolerance: self.closure_tolerance,
            block_closure_set: self.block_closure_set,
            whole_parent_closure: self.whole_parent_closure,
            whole_parent_closure_limits: self.whole_parent_closure_limits,
            clearance: self.clearance,
            source_authority,
            limits: self.transport_limits,
        }
    }
}

/// Complete live input. Direction bits are borrowed and never retained.
pub struct CommonArticulationCompactPairGeneralCellTransportInputV2<'a> {
    pub compact_authority: &'a GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    pub direction_bits_le: &'a [u8],
    pub foldability_source: GlobalFlatFoldabilityInput<'a>,
    pub source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    pub live: CommonArticulationCompactPairGeneralCellTransportLiveInputV2<'a>,
    pub limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
}

/// Replay uses the same complete live tuple as issuance.
pub type CommonArticulationCompactPairGeneralCellTransportRevalidationInputV2<'a> =
    CommonArticulationCompactPairGeneralCellTransportInputV2<'a>;

/// Retained no-search-origin receipt around the ordinary unpromoted outcome.
///
/// This value owns neither the compact authority nor the direction bytes. It
/// has no `Clone`, persistence, V1 conversion, or authorizing API.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2,
///     CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
/// };
/// fn accepts_v1(_: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1) {}
/// fn rejects_v2(value: CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2) {
///     accepts_v1(value);
/// }
/// ```
#[derive(Debug)]
pub struct CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2 {
    compact_provenance: GlobalFlatFoldabilityProvenance,
    variable_count: usize,
    variable_registry_sha256: [u8; 32],
    direction_assignment_sha256: [u8; 32],
    compact_work_counts: GlobalFlatFoldabilityWorkCounts,
    compact_limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
    compact_resources: GlobalFlatLayerOrderCompactPairAssignmentResourcesV2,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
    resources: CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    inner_transport_binding: [u8; 32],
    binding_fingerprint: [u8; 32],
    transport: CommonArticulationGeneralCellTransportOutcomeV2,
}

impl CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_COMPACT_PAIR_GENERAL_CELL_TRANSPORT_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn direction_assignment_sha256_v2(&self) -> [u8; 32] {
        self.direction_assignment_sha256
    }

    #[must_use]
    pub const fn variable_count_v2(&self) -> usize {
        self.variable_count
    }

    #[must_use]
    pub const fn variable_registry_sha256_v2(&self) -> [u8; 32] {
        self.variable_registry_sha256
    }

    #[must_use]
    pub const fn resources_v2(
        &self,
    ) -> CommonArticulationCompactPairGeneralCellTransportResourcesV2 {
        self.resources
    }

    #[must_use]
    pub fn transport_outcome_v2(&self) -> &CommonArticulationGeneralCellTransportOutcomeV2 {
        &self.transport
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationCompactPairGeneralCellTransportRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationCompactPairGeneralCellTransportRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
    ) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
        bridge_checkpoint_v2(&mut checkpoint)?;
        validate_compact_shape_v2(&input, &mut checkpoint)?;
        let admitted_envelope = checked_bridge_envelope_preflight_v2(&input)?;
        let direction_assignment_sha256 =
            validate_direction_assignment_binding_v2(&input, &mut checkpoint)?;
        let current_resources = checked_bridge_resources_v2(
            input.compact_authority,
            input.direction_bits_le.len(),
            input.source_revalidation_limits,
            self.transport.as_unpromoted_v2(),
            input.limits,
        )?;
        debug_assert!(current_resources.logical_work <= admitted_envelope.logical_work);
        debug_assert!(current_resources.retained_bytes <= admitted_envelope.retained_bytes);
        debug_assert!(current_resources.peak_bytes <= admitted_envelope.peak_bytes);
        let compact_matches = self.compact_provenance == input.compact_authority.provenance_v2()
            && self.variable_count == input.compact_authority.variable_count_v2()
            && self.variable_registry_sha256
                == input.compact_authority.variable_registry_sha256_v2()
            && self.direction_assignment_sha256 == direction_assignment_sha256
            && self.compact_work_counts == input.compact_authority.work_counts_v2()
            && self.compact_limits == input.compact_authority.exact_limits_v2()
            && self.compact_resources == input.compact_authority.resources_v2()
            && self.source_revalidation_limits == input.source_revalidation_limits
            && self.limits == input.limits
            && self.resources == current_resources
            && self.inner_transport_binding
                == self.transport.as_unpromoted_v2().binding_fingerprint_v2();
        if !compact_matches {
            return Err(
                CommonArticulationCompactPairGeneralCellTransportErrorV2::PrerequisiteBindingMismatch,
            );
        }

        let source_authority = revalidate_compact_source_v2(&input, &mut checkpoint)?;
        self.transport
            .as_unpromoted_v2()
            .revalidate_with_checkpoint_v2(
                input.live.revalidation_input(source_authority),
                &mut checkpoint,
            )
            .map_err(map_transport_error_v2)?;
        let binding = compact_pair_transport_binding_v2(
            input.compact_authority,
            input.source_revalidation_limits,
            input.limits,
            current_resources,
            self.transport.as_unpromoted_v2(),
            &mut checkpoint,
        )?;
        bridge_checkpoint_v2(&mut checkpoint)?;
        if binding != self.binding_fingerprint {
            return Err(
                CommonArticulationCompactPairGeneralCellTransportErrorV2::PrerequisiteBindingMismatch,
            );
        }
        Ok(())
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

/// The bridge can only retain an explicitly unpromoted observation.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationCompactPairGeneralCellTransportOutcomeV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationCompactPairGeneralCellTransportOutcomeV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationCompactPairGeneralCellTransportOutcomeV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationCompactPairGeneralCellTransportOutcomeV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationCompactPairGeneralCellTransportOutcomeV2,
///     CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
/// };
/// fn accepts_v1(_: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1) {}
/// fn rejects_v2(value: CommonArticulationCompactPairGeneralCellTransportOutcomeV2) {
///     accepts_v1(value);
/// }
/// ```
#[derive(Debug)]
pub enum CommonArticulationCompactPairGeneralCellTransportOutcomeV2 {
    Unpromoted(Box<CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2>),
}

impl CommonArticulationCompactPairGeneralCellTransportOutcomeV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_COMPACT_PAIR_GENERAL_CELL_TRANSPORT_MODEL_ID_V2
    }

    #[must_use]
    pub const fn is_certified_v2(&self) -> bool {
        false
    }

    #[must_use]
    pub fn as_unpromoted_v2(
        &self,
    ) -> &CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2 {
        match self {
            Self::Unpromoted(value) => value,
        }
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

/// Issues an unpromoted General-N observation from a live-revalidated compact
/// pair authority. No completion search is invoked by this bridge.
pub fn issue_common_articulation_compact_pair_general_cell_transport_prerequisite_v2(
    input: CommonArticulationCompactPairGeneralCellTransportInputV2<'_>,
) -> Result<
    CommonArticulationCompactPairGeneralCellTransportOutcomeV2,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    issue_common_articulation_compact_pair_general_cell_transport_prerequisite_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

/// Checkpoint-enabled form of
/// [`issue_common_articulation_compact_pair_general_cell_transport_prerequisite_v2`].
pub fn issue_common_articulation_compact_pair_general_cell_transport_prerequisite_with_checkpoint_v2(
    input: CommonArticulationCompactPairGeneralCellTransportInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<
    CommonArticulationCompactPairGeneralCellTransportOutcomeV2,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    bridge_checkpoint_v2(&mut checkpoint)?;
    validate_compact_shape_v2(&input, &mut checkpoint)?;
    let admitted_envelope = checked_bridge_envelope_preflight_v2(&input)?;
    let direction_assignment_sha256 =
        validate_direction_assignment_binding_v2(&input, &mut checkpoint)?;
    debug_assert_eq!(
        direction_assignment_sha256,
        input.compact_authority.direction_assignment_sha256_v2()
    );
    let source_authority = revalidate_compact_source_v2(&input, &mut checkpoint)?;
    let transport =
        issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(
            input.live.issue_input(source_authority),
            &mut checkpoint,
        )
        .map_err(map_transport_error_v2)?;
    let inner = transport.as_unpromoted_v2();
    let resources = checked_bridge_resources_v2(
        input.compact_authority,
        input.direction_bits_le.len(),
        input.source_revalidation_limits,
        inner,
        input.limits,
    )?;
    debug_assert!(resources.logical_work <= admitted_envelope.logical_work);
    debug_assert!(resources.retained_bytes <= admitted_envelope.retained_bytes);
    debug_assert!(resources.peak_bytes <= admitted_envelope.peak_bytes);
    let inner_transport_binding = inner.binding_fingerprint_v2();
    let binding_fingerprint = compact_pair_transport_binding_v2(
        input.compact_authority,
        input.source_revalidation_limits,
        input.limits,
        resources,
        inner,
        &mut checkpoint,
    )?;
    bridge_checkpoint_v2(&mut checkpoint)?;
    Ok(
        CommonArticulationCompactPairGeneralCellTransportOutcomeV2::Unpromoted(Box::new(
            CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2 {
                compact_provenance: input.compact_authority.provenance_v2(),
                variable_count: input.compact_authority.variable_count_v2(),
                variable_registry_sha256: input.compact_authority.variable_registry_sha256_v2(),
                direction_assignment_sha256,
                compact_work_counts: input.compact_authority.work_counts_v2(),
                compact_limits: input.compact_authority.exact_limits_v2(),
                compact_resources: input.compact_authority.resources_v2(),
                source_revalidation_limits: input.source_revalidation_limits,
                limits: input.limits,
                resources,
                inner_transport_binding,
                binding_fingerprint,
                transport,
            },
        )),
    )
}

fn validate_compact_shape_v2(
    input: &CommonArticulationCompactPairGeneralCellTransportInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    bridge_checkpoint_v2(checkpoint)?;
    if !bridge_limits_are_finite_v2(input.limits)
        || !revalidation_limits_are_finite_v2(input.source_revalidation_limits)
    {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::InvalidLimits);
    }
    let compact = input.compact_authority;
    let snapshot = compact.layer_order_snapshot_v2();
    let resources = compact.resources_v2();
    let compact_limits = compact.exact_limits_v2();
    let work = compact.work_counts_v2();
    let assignment_bytes = input.direction_bits_le.len();
    let expected_assignment_bytes = compact
        .variable_count_v2()
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment)?;
    if assignment_bytes != expected_assignment_bytes {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment);
    }
    if !compact.variable_count_v2().is_multiple_of(8) {
        let used = compact.variable_count_v2() % 8;
        let tail_mask = !((1_u8 << used) - 1);
        if input.direction_bits_le.last().copied().unwrap_or_default() & tail_mask != 0 {
            return Err(
                CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment,
            );
        }
    }
    if assignment_bytes > input.limits.max_compact_assignment_bytes
        || resources.compact_assignment_bytes != assignment_bytes
        || resources.layer_order_retained_bytes > compact_limits.max_layer_order_retained_bytes
        || resources.observed_peak_bytes > compact_limits.max_peak_bytes
        || resources.borrowed_live_bytes < assignment_bytes
    {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit);
    }
    if work.search_nodes != 0
        || work.overlap_face_pairs != compact.variable_count_v2()
        || snapshot.proof_summary.map(|summary| summary.search_nodes) != Some(0)
    {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::NoSearchInvariant);
    }
    if compact.variable_count_v2() != snapshot.face_pair_orders.len()
        || compact.provenance_v2() != snapshot.provenance.source
        || !snapshot.is_current_for(&compact.provenance_v2())
    {
        return Err(
            CommonArticulationCompactPairGeneralCellTransportErrorV2::CompactSourceBindingMismatch,
        );
    }
    Ok(())
}

fn validate_direction_assignment_binding_v2(
    input: &CommonArticulationCompactPairGeneralCellTransportInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<[u8; 32], CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    let compact = input.compact_authority;
    let digest = direction_assignment_digest_with_checkpoint_v2(
        compact.variable_count_v2(),
        compact.variable_registry_sha256_v2(),
        input.direction_bits_le,
        checkpoint,
    )?;
    if digest != compact.direction_assignment_sha256_v2() {
        return Err(
            CommonArticulationCompactPairGeneralCellTransportErrorV2::CompactSourceBindingMismatch,
        );
    }
    Ok(digest)
}

fn revalidate_compact_source_v2<'a>(
    input: &CommonArticulationCompactPairGeneralCellTransportInputV2<'a>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<
    GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    let source_authority = {
        let mut observer = BridgeFoldabilityObserverV2 { checkpoint };
        revalidate_global_flat_layer_order_source_with_observer_v2(
            input.foldability_source,
            input.compact_authority.layer_order_snapshot_v2(),
            input.source_revalidation_limits,
            &mut observer,
        )
        .map_err(map_source_revalidation_error_v2)?
    };
    if source_authority.provenance_v2() != input.compact_authority.provenance_v2()
        || !source_authority.is_current_v2()
    {
        return Err(
            CommonArticulationCompactPairGeneralCellTransportErrorV2::CompactSourceBindingMismatch,
        );
    }
    Ok(source_authority)
}

struct BridgeFoldabilityObserverV2<'a, C> {
    checkpoint: &'a mut C,
}

impl<C> GlobalFlatFoldabilityObserver for BridgeFoldabilityObserverV2<'_, C>
where
    C: FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
{
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        match (self.checkpoint)() {
            Ok(()) => GlobalFlatFoldabilityCheckpoint::Continue,
            Err(CommonArticulationGeneralCellTransportStopV2::Cancelled) => {
                GlobalFlatFoldabilityCheckpoint::Cancelled
            }
            Err(CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded) => {
                GlobalFlatFoldabilityCheckpoint::DeadlineReached
            }
        }
    }
}

fn direction_assignment_digest_with_checkpoint_v2(
    variable_count: usize,
    variable_registry_sha256: [u8; 32],
    direction_bits_le: &[u8],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<[u8; 32], CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    let expected_bytes = variable_count
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment)?;
    if direction_bits_le.len() != expected_bytes {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment);
    }
    if !variable_count.is_multiple_of(8) {
        let used = variable_count % 8;
        let tail_mask = !((1_u8 << used) - 1);
        if direction_bits_le.last().copied().unwrap_or_default() & tail_mask != 0 {
            return Err(
                CommonArticulationCompactPairGeneralCellTransportErrorV2::MalformedAssignment,
            );
        }
    }
    let variable_count_u64 = u64::try_from(variable_count)
        .map_err(|_| CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    let assignment_bytes_u64 = u64::try_from(direction_bits_le.len())
        .map_err(|_| CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    let mut hash = Sha256::new();
    hash.update(GLOBAL_FLAT_LAYER_ORDER_COMPACT_PAIR_ASSIGNMENT_DOMAIN_V2);
    hash.update(variable_count_u64.to_le_bytes());
    hash.update(variable_registry_sha256);
    hash.update(assignment_bytes_u64.to_le_bytes());
    for chunk in direction_bits_le.chunks(DIRECTION_HASH_CHUNK_BYTES_V2) {
        bridge_checkpoint_v2(checkpoint)?;
        hash.update(chunk);
    }
    bridge_checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn checked_bridge_resources_v2(
    compact: &GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    assignment_bytes: usize,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    inner: &CommonArticulationGeneralCellTransportPrerequisiteV2,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
) -> Result<
    CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    checked_bridge_resource_values_v2(
        compact,
        assignment_bytes,
        source_revalidation_limits,
        inner.logical_work_v2(),
        inner.retained_bytes_v2(),
        inner.peak_bytes_v2(),
        limits,
    )
}

/// Rejects an outer bridge envelope before live source revalidation or inner
/// transport can allocate a candidate. The delegated transport's admitted
/// maxima are charged conservatively; a later pass binds the actual outcome.
fn checked_bridge_envelope_preflight_v2(
    input: &CommonArticulationCompactPairGeneralCellTransportInputV2<'_>,
) -> Result<
    CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    // The retained snapshot size was measured while the sealed compact
    // authority was issued and cannot change. Reject both known-impossible
    // revalidation envelopes before hashing borrowed direction bytes or
    // entering live validation, whose first phase may allocate.
    let compact_source_bytes = input
        .compact_authority
        .resources_v2()
        .layer_order_retained_bytes;
    if compact_source_bytes > input.source_revalidation_limits.max_source_retained_bytes
        || compact_source_bytes > input.source_revalidation_limits.max_peak_bytes
    {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit);
    }
    checked_bridge_resource_values_v2(
        input.compact_authority,
        input.direction_bits_le.len(),
        input.source_revalidation_limits,
        input.live.transport_limits.max_logical_work,
        input.live.transport_limits.max_retained_bytes,
        input.live.transport_limits.max_peak_bytes,
        input.limits,
    )
}

fn checked_bridge_resource_values_v2(
    compact: &GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    assignment_bytes: usize,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    transport_logical_work: usize,
    transport_retained_bytes: usize,
    transport_peak_bytes: usize,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
) -> Result<
    CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    CommonArticulationCompactPairGeneralCellTransportErrorV2,
> {
    let authority_shell_bytes = size_of::<GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2>()
        .checked_sub(size_of::<LayerOrderSnapshot>())
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    let compact_source_retained_bytes = compact
        .resources_v2()
        .layer_order_retained_bytes
        .checked_add(authority_shell_bytes)
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    let logical_work = checked_compact_work_v2(compact.work_counts_v2())?
        .checked_add(assignment_bytes)
        .and_then(|value| value.checked_add(transport_logical_work))
        .and_then(|value| value.checked_add(COMPACT_PAIR_TRANSPORT_BASE_WORK_V2))
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    let outer_retained_bytes =
        size_of::<CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2>();
    let retained_bytes = transport_retained_bytes
        .checked_add(outer_retained_bytes)
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    // Source revalidation and delegated transport are sequential. Both charge
    // the shared retained snapshot already; only compact metadata, borrowed
    // direction bytes, the retained wrapper, and its hash workspace are added.
    let peak_bytes = source_revalidation_limits
        .max_peak_bytes
        .max(transport_peak_bytes)
        .checked_add(authority_shell_bytes)
        .and_then(|value| value.checked_add(assignment_bytes))
        .and_then(|value| value.checked_add(outer_retained_bytes))
        .and_then(|value| value.checked_add(COMPACT_PAIR_TRANSPORT_WORKSPACE_BYTES_V2))
        .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    if assignment_bytes > limits.max_compact_assignment_bytes
        || compact_source_retained_bytes > limits.max_compact_source_retained_bytes
        || logical_work > limits.max_logical_work
        || retained_bytes > limits.max_retained_bytes
        || peak_bytes > limits.max_peak_bytes
    {
        return Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit);
    }
    Ok(
        CommonArticulationCompactPairGeneralCellTransportResourcesV2 {
            compact_assignment_bytes: assignment_bytes,
            compact_source_retained_bytes,
            source_revalidation_peak_bytes: source_revalidation_limits.max_peak_bytes,
            transport_logical_work,
            transport_retained_bytes,
            transport_peak_bytes,
            logical_work,
            retained_bytes,
            peak_bytes,
        },
    )
}

fn checked_compact_work_v2(
    work: GlobalFlatFoldabilityWorkCounts,
) -> Result<usize, CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    [
        work.source_vertex_records,
        work.source_edge_records,
        work.paper_boundary_vertex_records,
        work.face_records,
        work.face_boundary_half_edges,
        work.hinge_records,
        work.edge_incidence_records,
        work.local_vertex_records,
        work.total_records,
        work.overlap_face_pairs,
        work.arrangement_segments,
        work.overlap_cells,
        work.constraints,
        work.search_nodes,
        work.exact_operations,
        work.exact_values,
        work.certificate_bytes,
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)
}

fn compact_pair_transport_binding_v2(
    compact: &GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
    resources: CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    inner: &CommonArticulationGeneralCellTransportPrerequisiteV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<[u8; 32], CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_COMPACT_PAIR_GENERAL_CELL_TRANSPORT_MODEL_ID_V2.as_bytes());
    hash.update(GLOBAL_FLAT_LAYER_ORDER_COMPACT_PAIR_ASSIGNMENT_DOMAIN_V2);
    hash.update(COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_MODEL_ID_V2.as_bytes());
    hash.update(compact.variable_registry_sha256_v2());
    hash.update(compact.direction_assignment_sha256_v2());
    hash.update(inner.binding_fingerprint_v2());
    hash.update(inner.source_digest_v2());
    hash_provenance_v2(&mut hash, compact.provenance_v2());
    hash_usize_bridge_v2(&mut hash, compact.variable_count_v2())?;
    hash_work_counts_v2(&mut hash, compact.work_counts_v2(), checkpoint)?;
    hash_compact_limits_v2(&mut hash, compact.exact_limits_v2(), checkpoint)?;
    hash_compact_resources_v2(&mut hash, compact.resources_v2(), checkpoint)?;
    hash_revalidation_limits_v2(&mut hash, source_revalidation_limits, checkpoint)?;
    hash_bridge_limits_v2(&mut hash, limits, checkpoint)?;
    hash_bridge_resources_v2(&mut hash, resources, checkpoint)?;
    for value in [
        inner.actual_block_count_v2(),
        inner.logical_work_v2(),
        inner.retained_bytes_v2(),
        inner.peak_bytes_v2(),
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(&mut hash, value)?;
    }
    bridge_checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn hash_provenance_v2(hash: &mut Sha256, provenance: GlobalFlatFoldabilityProvenance) {
    hash.update([match provenance.model_id {
        ori_foldability::GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1 => 1,
    }]);
    match provenance.identity_namespace {
        Some(namespace) => {
            hash.update([1]);
            hash.update(namespace.canonical_bytes());
        }
        None => hash.update([0]),
    }
    hash.update(provenance.source_revision.to_le_bytes());
    match provenance.source_fingerprint {
        Some(fingerprint) => {
            hash.update([1]);
            hash.update(fingerprint.0);
        }
        None => hash.update([0]),
    }
}

fn hash_work_counts_v2(
    hash: &mut Sha256,
    work: GlobalFlatFoldabilityWorkCounts,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    for value in [
        work.source_vertex_records,
        work.source_edge_records,
        work.paper_boundary_vertex_records,
        work.face_records,
        work.face_boundary_half_edges,
        work.hinge_records,
        work.edge_incidence_records,
        work.local_vertex_records,
        work.total_records,
        work.overlap_face_pairs,
        work.arrangement_segments,
        work.overlap_cells,
        work.constraints,
        work.search_nodes,
        work.exact_operations,
        work.exact_values,
        work.certificate_bytes,
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_analysis_limits_v2(
    hash: &mut Sha256,
    limits: GlobalFlatFoldabilityLimits,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    for value in analysis_limit_values_v2(limits) {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_compact_limits_v2(
    hash: &mut Sha256,
    limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    hash_analysis_limits_v2(hash, limits.analysis, checkpoint)?;
    for value in [
        limits.max_compact_assignment_bytes,
        limits.max_layer_order_retained_bytes,
        limits.max_peak_bytes,
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_compact_resources_v2(
    hash: &mut Sha256,
    resources: GlobalFlatLayerOrderCompactPairAssignmentResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    for value in [
        resources.compact_assignment_bytes,
        resources.borrowed_live_bytes,
        resources.layer_order_retained_bytes,
        resources.observed_validation_peak_bytes,
        resources.observed_facewise_peak_bytes,
        resources.observed_peak_bytes,
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_revalidation_limits_v2(
    hash: &mut Sha256,
    limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    hash_analysis_limits_v2(hash, limits.analysis, checkpoint)?;
    for value in [limits.max_source_retained_bytes, limits.max_peak_bytes] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_bridge_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    for value in [
        limits.max_compact_assignment_bytes,
        limits.max_compact_source_retained_bytes,
        limits.max_logical_work,
        limits.max_retained_bytes,
        limits.max_peak_bytes,
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn hash_bridge_resources_v2(
    hash: &mut Sha256,
    resources: CommonArticulationCompactPairGeneralCellTransportResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    for value in [
        resources.compact_assignment_bytes,
        resources.compact_source_retained_bytes,
        resources.source_revalidation_peak_bytes,
        resources.transport_logical_work,
        resources.transport_retained_bytes,
        resources.transport_peak_bytes,
        resources.logical_work,
        resources.retained_bytes,
        resources.peak_bytes,
    ] {
        bridge_checkpoint_v2(checkpoint)?;
        hash_usize_bridge_v2(hash, value)?;
    }
    Ok(())
}

fn analysis_limit_values_v2(limits: GlobalFlatFoldabilityLimits) -> [usize; 17] {
    [
        limits.max_source_vertices,
        limits.max_source_edges,
        limits.max_paper_boundary_vertices,
        limits.max_faces,
        limits.max_face_boundary_half_edges,
        limits.max_hinges,
        limits.max_edge_incidence_records,
        limits.max_local_vertices,
        limits.max_total_records,
        limits.max_overlap_face_pairs,
        limits.max_arrangement_segments,
        limits.max_overlap_cells,
        limits.max_constraints,
        limits.max_search_nodes,
        limits.max_exact_integer_bits,
        limits.max_exact_operations,
        limits.max_certificate_bytes,
    ]
}

fn bridge_limits_are_finite_v2(
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
) -> bool {
    ![
        limits.max_compact_assignment_bytes,
        limits.max_compact_source_retained_bytes,
        limits.max_logical_work,
        limits.max_retained_bytes,
        limits.max_peak_bytes,
    ]
    .contains(&usize::MAX)
}

fn revalidation_limits_are_finite_v2(limits: GlobalFlatLayerOrderRevalidationLimitsV2) -> bool {
    !analysis_limit_values_v2(limits.analysis).contains(&usize::MAX)
        && limits.max_source_retained_bytes != usize::MAX
        && limits.max_peak_bytes != usize::MAX
}

fn hash_usize_bridge_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    let value = u64::try_from(value)
        .map_err(|_| CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit)?;
    hash.update(value.to_le_bytes());
    Ok(())
}

fn bridge_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationCompactPairGeneralCellTransportErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationGeneralCellTransportStopV2::Cancelled => {
            CommonArticulationCompactPairGeneralCellTransportErrorV2::Cancelled
        }
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
            CommonArticulationCompactPairGeneralCellTransportErrorV2::DeadlineExceeded
        }
    })
}

fn map_source_revalidation_error_v2(
    error: GlobalFlatLayerOrderRevalidationErrorV2,
) -> CommonArticulationCompactPairGeneralCellTransportErrorV2 {
    match error {
        GlobalFlatLayerOrderRevalidationErrorV2::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled,
        ) => CommonArticulationCompactPairGeneralCellTransportErrorV2::Cancelled,
        GlobalFlatLayerOrderRevalidationErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. },
        } => CommonArticulationCompactPairGeneralCellTransportErrorV2::DeadlineExceeded,
        error => {
            CommonArticulationCompactPairGeneralCellTransportErrorV2::SourceRevalidation(error)
        }
    }
}

fn map_transport_error_v2(
    error: CommonArticulationGeneralCellTransportErrorV2,
) -> CommonArticulationCompactPairGeneralCellTransportErrorV2 {
    match error {
        CommonArticulationGeneralCellTransportErrorV2::Cancelled => {
            CommonArticulationCompactPairGeneralCellTransportErrorV2::Cancelled
        }
        CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded => {
            CommonArticulationCompactPairGeneralCellTransportErrorV2::DeadlineExceeded
        }
        error => CommonArticulationCompactPairGeneralCellTransportErrorV2::Transport(error),
    }
}

#[cfg(test)]
mod tests;
