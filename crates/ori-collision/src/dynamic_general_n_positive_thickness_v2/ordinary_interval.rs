//! Outward-interval swept-AABB proof for ordinary face pairs.
//!
//! The kernel builds its own canonical dyadic collision partition from the
//! live schedule. It does not inspect or reinterpret the opaque dynamic
//! closure bridge partition. Every excluded pair is reclassified directly
//! from live face-boundary vertex identities, so the exclusion registry
//! cannot hide an ordinary collision and shared pairs may be local or
//! cross-block.

use std::cmp::Ordering;

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationDynamicClosureBridgeV2,
    CommonArticulationDynamicClosureIntervalTransformLeafV2,
    CommonArticulationDynamicClosureIntervalTransformSessionV2, CommonArticulationPoseAuthorityV2,
    CommonArticulationResourceProfileV2, CycleScheduleDyadicWorkspaceBoundV2,
    CycleScheduleLimitsV1, IntervalFaceTransformWorkspaceBoundV2, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};
use sha2::{Digest, Sha256};

mod binding;
mod geometry;
mod partition;
#[path = "ordinary_interval/public_adapter.rs"]
pub(crate) mod public_adapter;
mod relief_aggregate;
mod resources;

const ORDINARY_INTERVAL_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_ordinary_interval_clearance_v2";
const AXIS_COUNT_V2: usize = 3;
const THICK_SURFACE_COUNT_V2: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OrdinaryIntervalStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OrdinaryIntervalErrorV2 {
    InvalidInput,
    ResourceLimit,
    NonCanonicalExcludedSharedPairRegistry,
    DuplicateExcludedSharedPair,
    ExcludedSharedPairCoverageMismatch,
    UnprovenOrdinaryClearance,
    Cancelled,
    DeadlineExceeded,
}

/// Canonical unordered pair used only inside the sealed Phase 3E aggregation
/// boundary. It has no public constructor or accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OrdinaryIntervalFacePairV2 {
    first: FaceId,
    second: FaceId,
}

impl OrdinaryIntervalFacePairV2 {
    pub(super) fn new(first: FaceId, second: FaceId) -> Option<Self> {
        match first.canonical_bytes().cmp(&second.canonical_bytes()) {
            Ordering::Less => Some(Self { first, second }),
            Ordering::Greater => Some(Self {
                first: second,
                second: first,
            }),
            Ordering::Equal => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OrdinaryIntervalLimitsV2 {
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_boundary_vertex_occurrences: usize,
    pub max_excluded_shared_pairs: usize,
    pub max_shared_feature_membership_tests: usize,
    pub max_collision_depth: u32,
    pub max_collision_leaves: usize,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub max_bridge_retained_bytes: usize,
    pub max_bridge_revalidation_peak_bytes: usize,
    pub max_schedule_retained_bytes: usize,
    pub max_session_shell_bytes: usize,
    pub max_schedule_evaluation_workspace_bytes: usize,
    pub max_bridge_partition_search_work_per_node: usize,
    pub max_interval_transform_work_per_node: usize,
    pub max_interval_registry_validation_work_per_node: usize,
    pub max_interval_registry_sort_comparisons_per_node: usize,
    pub max_interval_registry_workspace_bytes: usize,
    pub max_interval_registry_retained_bytes: usize,
    pub max_ordinary_pair_node_tests: usize,
    /// Maximum work charged to nine collision-kernel categories: shared-pair
    /// membership, schedule evaluation, interval-transform complexity,
    /// bridge-partition search, registry validation, registry sorting,
    /// thick-surface visits, pair classification and axis separation tests.
    /// Bridge replay, schedule metadata scans, base-input validation, input
    /// hashing, structural face/pair/stack bookkeeping, partition hashing and
    /// scalar publication/binding work are outside this phase-specific
    /// counter. Each remains finite under the submitted cardinality limits or
    /// the dedicated issuer/resource policy for its phase.
    pub max_logical_work: usize,
    /// Peak of the proof-carrier allocation ledger, not whole-process RSS.
    /// Caller-owned immutable geometry, audit, pose, decomposition,
    /// common-pose and profile backing is excluded and remains governed by
    /// the corresponding input/issuer caps.
    pub max_temporary_bytes: usize,
    pub max_publication_bytes: usize,
    /// Maximum of the temporary and publication phases under the same
    /// proof-carrier ledger and exclusions as `max_temporary_bytes`.
    pub max_aggregate_peak_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OrdinaryIntervalResourcesV2 {
    face_count: usize,
    hinge_count: usize,
    boundary_vertex_occurrences: usize,
    total_face_pairs: usize,
    excluded_shared_pairs: usize,
    ordinary_face_pairs: usize,
    charged_interval_nodes: usize,
    charged_shared_feature_membership_tests: usize,
    charged_ordinary_pair_node_tests: usize,
    charged_axis_tests: usize,
    charged_surface_vertex_visits: usize,
    charged_interval_registry_validation_work: usize,
    charged_interval_registry_sort_comparisons: usize,
    /// The same nine-category phase-specific contract documented by
    /// `max_logical_work`; this is not a whole-call CPU counter, and the
    /// enumerated structural/hash/publication work remains governed by its
    /// finite cardinality or issuer policy instead.
    charged_logical_work: usize,
    charged_pending_partition_bytes: usize,
    charged_bridge_retained_bytes: usize,
    charged_bridge_revalidation_peak_bytes: usize,
    charged_schedule_retained_bytes: usize,
    charged_session_shell_bytes: usize,
    charged_session_steady_retained_bytes: usize,
    charged_bridge_revalidation_phase_peak_bytes: usize,
    charged_bridge_partition_search_work: usize,
    charged_schedule_evaluation_workspace_bytes: usize,
    charged_angle_box_bytes: usize,
    charged_interval_registry_workspace_bytes: usize,
    charged_interval_registry_retained_bytes: usize,
    charged_leaf_wrapper_overhead_bytes: usize,
    charged_leaf_retained_bytes: usize,
    charged_face_aabb_bytes: usize,
    charged_temporary_bytes: usize,
    charged_publication_bytes: usize,
    charged_aggregate_peak_bytes: usize,
}

/// Private proof material. No leaf, pair, transform, or lower certificate is
/// retained; a later aggregate revalidation must rerun this kernel completely.
#[derive(Debug)]
pub(super) struct OrdinaryIntervalEvidenceV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    audit_binding: [u8; 32],
    schedule_binding: [u8; 32],
    bridge_binding: [u8; 32],
    excluded_shared_pair_digest: [u8; 32],
    collision_partition_digest: [u8; 32],
    binding_fingerprint: [u8; 32],
    fixed_face: FaceId,
    thickness_bits: u64,
    closure_tolerance_bits: u64,
    accepted_leaf_count: usize,
    processed_interval_node_count: usize,
    maximum_accepted_depth: u32,
    certified_ordinary_pair_leaf_count: usize,
    resources: OrdinaryIntervalResourcesV2,
    limits: OrdinaryIntervalLimitsV2,
}

#[derive(Clone, Copy)]
pub(super) struct OrdinaryIntervalInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub fixed_face: FaceId,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub dynamic_closure_bridge: &'a CommonArticulationDynamicClosureBridgeV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub excluded_shared_pairs: &'a [OrdinaryIntervalFacePairV2],
    pub limits: OrdinaryIntervalLimitsV2,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DyadicLeafV2 {
    depth: u32,
    index: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ThickFaceAabbV2 {
    face: FaceId,
    lower: [f64; AXIS_COUNT_V2],
    upper: [f64; AXIS_COUNT_V2],
}

pub(super) struct ValidatedInputV2<'a> {
    audit_binding: [u8; 32],
    excluded_shared_pair_digest: [u8; 32],
    resources: OrdinaryIntervalResourcesV2,
    schedule_workspace_bound: CycleScheduleDyadicWorkspaceBoundV2,
    interval_transform_workspace_bound: IntervalFaceTransformWorkspaceBoundV2,
    interval_transform_session: CommonArticulationDynamicClosureIntervalTransformSessionV2<'a>,
}

pub(super) struct ProofRunV2 {
    collision_partition_digest: [u8; 32],
    accepted_leaf_count: usize,
    processed_interval_node_count: usize,
    maximum_accepted_depth: u32,
    certified_ordinary_pair_leaf_count: usize,
}

pub(super) fn prove_ordinary_interval_clearance_v2(
    input: OrdinaryIntervalInputV2<'_>,
) -> Result<OrdinaryIntervalEvidenceV2, OrdinaryIntervalErrorV2> {
    prove_ordinary_interval_clearance_with_checkpoint_v2(input, || Ok(()))
}

pub(super) fn prove_ordinary_interval_clearance_with_checkpoint_v2(
    input: OrdinaryIntervalInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<OrdinaryIntervalEvidenceV2, OrdinaryIntervalErrorV2> {
    checkpoint_v2(&mut checkpoint)?;
    let validated = resources::validate_input_v2(&input, &mut checkpoint)?;
    let run = partition::prove_partition_v2(&input, &validated, &mut checkpoint)?;
    checkpoint_v2(&mut checkpoint)?;
    let binding_fingerprint = binding::binding_fingerprint_v2(&input, &validated, &run)?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(OrdinaryIntervalEvidenceV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        audit_binding: validated.audit_binding,
        schedule_binding: input.schedule.certificate_binding_fingerprint_v2(),
        bridge_binding: validated
            .interval_transform_session
            .bridge_binding_fingerprint_v2(),
        excluded_shared_pair_digest: validated.excluded_shared_pair_digest,
        collision_partition_digest: run.collision_partition_digest,
        binding_fingerprint,
        fixed_face: input.fixed_face,
        thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        accepted_leaf_count: run.accepted_leaf_count,
        processed_interval_node_count: run.processed_interval_node_count,
        maximum_accepted_depth: run.maximum_accepted_depth,
        certified_ordinary_pair_leaf_count: run.certified_ordinary_pair_leaf_count,
        resources: validated.resources,
        limits: input.limits,
    })
}

fn compare_pair_v2(
    left: &OrdinaryIntervalFacePairV2,
    right: &OrdinaryIntervalFacePairV2,
) -> Ordering {
    left.first
        .canonical_bytes()
        .cmp(&right.first.canonical_bytes())
        .then_with(|| {
            left.second
                .canonical_bytes()
                .cmp(&right.second.canonical_bytes())
        })
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Result<(), OrdinaryIntervalErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| OrdinaryIntervalErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    checkpoint().map_err(|stop| match stop {
        OrdinaryIntervalStopV2::Cancelled => OrdinaryIntervalErrorV2::Cancelled,
        OrdinaryIntervalStopV2::DeadlineExceeded => OrdinaryIntervalErrorV2::DeadlineExceeded,
    })
}

#[cfg(test)]
#[path = "ordinary_interval/tests.rs"]
pub(crate) mod tests;
