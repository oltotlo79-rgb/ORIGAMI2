use std::mem::size_of;

use ori_domain::EdgeId;
use ori_topology::FoldAssignment;
use sha2::{Digest, Sha256};

use super::*;
use crate::schedule::{
    CycleScheduleDyadicEvaluationErrorV2, CycleScheduleDyadicEvaluationStopV2,
    CycleScheduleDyadicWorkspaceBoundV2,
};

mod exact_parallel_cut;

use exact_parallel_cut::{
    ExactParallelCutRecognitionV2, recognize_exact_parallel_cut_with_checkpoint_v2,
};

/// Resource policy for the allocation-bounded, adaptive dyadic V2 engine.
///
/// This type deliberately remains crate-private until the general-N wrappers
/// define their public compatibility surface. Every byte field is a hard
/// ceiling; `usize::MAX` is rejected rather than treated as unbounded. The
/// caller-owned borrowed schedule's retained heap is outside this primitive's
/// accounting. A wrapper that owns or restricts a schedule must charge that
/// material and its construction peak separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DyadicIntervalClosureWorkspaceLimitsV2 {
    pub(crate) max_depth: u32,
    pub(crate) max_leaves: usize,
    pub(crate) max_work: usize,
    pub(crate) schedule_limits: CycleScheduleLimitsV1,
    pub(crate) max_theorem_recognizer_work: usize,
    pub(crate) max_theorem_recognizer_workspace_bytes: usize,
    pub(crate) max_carrier_index_workspace_bytes: usize,
    pub(crate) max_schedule_evaluation_workspace_bytes: usize,
    pub(crate) max_big_rational_payload_bytes: usize,
    pub(crate) max_exact_rational_object_bytes: usize,
    pub(crate) max_interval_closure_workspace_bytes: usize,
    pub(crate) max_partition_workspace_bytes: usize,
    pub(crate) max_retained_material_bytes: usize,
    pub(crate) max_publication_workspace_bytes: usize,
    pub(crate) max_peak_workspace_bytes: usize,
}

/// Charged ceilings retained with one successful V2 proof.
///
/// These values are not claimed to be allocator-independent minima. For each
/// phase the issuer records the greater of its checked preflight ceiling and
/// the physical capacities it can observe after fallible reservation. Rust's
/// reservation API reports capacity only after allocation, so an allocator may
/// briefly return an over-limit buffer before the issuer observes and rejects
/// it; no interval-closure arithmetic or material publication follows that
/// rejection. Deterministic carrier/adjacency construction may precede an
/// aggregate nested-capacity check. Big-rational payload and exact-object
/// fields are subceilings already contained in schedule-evaluation bytes and
/// must not be added to the overall peak a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DyadicIntervalClosureWorkspaceResourcesV2 {
    pub(crate) charged_binding_validation_upper_bound_bytes: usize,
    pub(crate) charged_theorem_recognizer_work: usize,
    pub(crate) charged_theorem_recognizer_upper_bound_bytes: usize,
    pub(crate) charged_carrier_index_workspace_upper_bound_bytes: usize,
    pub(crate) charged_schedule_evaluation_workspace_upper_bound_bytes: usize,
    pub(crate) charged_big_rational_payload_upper_bound_bytes: usize,
    pub(crate) charged_exact_rational_object_upper_bound_bytes: usize,
    pub(crate) charged_interval_closure_workspace_upper_bound_bytes: usize,
    pub(crate) charged_partition_workspace_upper_bound_bytes: usize,
    pub(crate) charged_retained_material_upper_bound_bytes: usize,
    pub(crate) charged_publication_workspace_upper_bound_bytes: usize,
    pub(crate) charged_peak_workspace_upper_bound_bytes: usize,
    pub(crate) visited_partition_nodes: usize,
    pub(crate) issued_leaves: usize,
}

/// Opaque closure material produced only by the workspace-bounded issuer.
///
/// The carrier is stored once rather than once per leaf. This is intentionally
/// `Debug`-only: it is neither cloneable, serializable, nor an authority token.
#[derive(Debug)]
pub(crate) struct WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    fixed_face: FaceId,
    schedule_binding_fingerprint_v2: [u8; 32],
    graph_binding_fingerprint_v1: [u8; 32],
    tolerance_bits: u64,
    policy: DyadicIntervalClosureWorkspaceLimitsV2,
    partition: Vec<(u32, u64)>,
    canonical_checked_hinges: Vec<EdgeId>,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    partition_binding_fingerprint_v2: [u8; 32],
}

impl WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
    #[must_use]
    pub(crate) const fn resources(&self) -> DyadicIntervalClosureWorkspaceResourcesV2 {
        self.resources
    }

    #[must_use]
    pub(crate) fn partition(&self) -> &[(u32, u64)] {
        &self.partition
    }

    #[must_use]
    pub(crate) fn canonical_checked_hinges(&self) -> &[EdgeId] {
        &self.canonical_checked_hinges
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn has_nonempty_canonical_complete_partition_v2(&self) -> bool {
        has_nonempty_canonical_complete_partition_v2(&self.partition)
    }

    /// Precomputed, domain-separated binding for the policy, complete
    /// partition and normalized all-hinge carrier.
    #[must_use]
    pub(crate) const fn partition_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.partition_binding_fingerprint_v2
    }
}

#[derive(Debug, Clone, Copy)]
struct WorkspacePreflightV2 {
    schedule: CycleScheduleDyadicWorkspaceBoundV2,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntervalAttemptErrorV2 {
    InvalidInput,
    ResourceLimit,
    Unproven,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug)]
pub(super) struct IntervalAttemptSuccessV2 {
    pub(super) physical_capacity_bytes: usize,
    pub(super) poses: Vec<Option<IntervalRigidTransformV1>>,
}

mod interval_closure;
mod issue;
mod preflight;
mod validation;

pub(super) use interval_closure::{
    IntervalClosureRequestV2, IntervalClosureVerificationModeV2,
    prove_interval_closure_with_workspace_v2,
};
use interval_closure::{is_spanning_v2, map_checkpoint_v2};
use preflight::{
    checked_preflight_v2, checked_vec_bytes_v2, limits_contain_usize_max_v2, refresh_peak_v2,
    resources_fit_limits_v2,
};
#[cfg(test)]
use validation::has_nonempty_canonical_complete_partition_v2;
use validation::{
    compute_partition_binding_with_checkpoint_v2, map_heap_sort_error_v2,
    map_interval_control_error_v2, split_partition_leaf_v2,
    validate_audit_order_with_checkpoint_v2, validate_carrier_with_checkpoint_v2,
    validate_partition_with_checkpoint_v2,
};

#[cfg(test)]
mod tests;
