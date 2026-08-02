//! Sealed interval-transform sessions backed by an all-domain closure bridge.
//!
//! A session revalidates the complete live bridge tuple once. Its leaf method
//! then evaluates the same canonical schedule internally and exposes only an
//! observation registry built from the audit spanning tree. The evaluated
//! angle carrier never crosses the crate boundary.
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureIntervalTransformSessionV2;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>>();
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureIntervalTransformSessionV2;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>>();
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureIntervalTransformSessionV2;
//!
//! fn require_deref<T: std::ops::Deref>() {}
//! require_deref::<CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>>();
//! ```

use std::fmt;

use ori_domain::FaceId;

use super::{
    CommonArticulationDynamicClosureBridgeErrorV2,
    CommonArticulationDynamicClosureBridgeRevalidationInputV2,
    CommonArticulationDynamicClosureBridgeStopV2, CommonArticulationDynamicClosureBridgeV2,
    checkpoint_bridge_v2,
};
use crate::graph::WorkspaceBoundedMaterialFaceTransformRegistryV2;
use crate::{
    CanonicalCycleScheduleV1, CycleScheduleDyadicEvaluationErrorV2,
    CycleScheduleDyadicEvaluationStopV2, CycleScheduleDyadicWorkspaceBoundV2,
    CycleScheduleLimitsV1, CycleSchedulePrepareErrorV1, DyadicIntervalClosureStopV1,
    IntervalFaceTransformWorkspaceBoundV2, IntervalFaceTransformWorkspaceErrorV2,
    IntervalFaceTransformWorkspaceResourcesV2, IntervalRigidTransformV1,
    MaterialHingeGraphGeometry,
};

mod coverage;
#[cfg(test)]
mod negative_trait_contracts;
mod resources;

use coverage::parent_partition_covers_leaf_v2;
use resources::checked_session_resources_with_checkpoint_v2;
pub use resources::{
    CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2,
    CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
};

/// Fail-closed result from one sealed session leaf observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicClosureIntervalTransformLeafErrorV2 {
    InvalidInput,
    ResourceLimit,
    /// Diagnostic observation that outward schedule or transform intervals
    /// require a narrower leaf. This is not a partition descriptor,
    /// certificate, authority, or proof artifact.
    Inconclusive,
    Cancelled,
    DeadlineExceeded,
}

/// Borrowed, non-cloneable session tied to one revalidated bridge tuple.
///
/// This value is an observation conduit only. It is not a closure
/// certificate, pose authority, or motion authorization.
pub struct CommonArticulationDynamicClosureIntervalTransformSessionV2<'a> {
    bridge: &'a CommonArticulationDynamicClosureBridgeV2,
    input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a>,
    resources: CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
}

impl fmt::Debug for CommonArticulationDynamicClosureIntervalTransformSessionV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommonArticulationDynamicClosureIntervalTransformSessionV2")
            .field("resources", &self.resources)
            .finish_non_exhaustive()
    }
}

/// One opaque diagnostic observation. No angle boxes, pose vector, partition
/// descriptor, certificate, authority, or registry backing can be extracted
/// from this value.
pub struct CommonArticulationDynamicClosureIntervalTransformLeafV2<'a> {
    geometry: &'a MaterialHingeGraphGeometry,
    registry: WorkspaceBoundedMaterialFaceTransformRegistryV2,
    resources: CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2,
}

impl fmt::Debug for CommonArticulationDynamicClosureIntervalTransformLeafV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommonArticulationDynamicClosureIntervalTransformLeafV2")
            .field("resources", &self.resources)
            .finish_non_exhaustive()
    }
}

impl CommonArticulationDynamicClosureIntervalTransformLeafV2<'_> {
    #[must_use]
    pub const fn resources(
        &self,
    ) -> CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2 {
        self.resources
    }

    /// Constant-time canonical-position lookup. Passing any geometry other
    /// than the exact session issuer fails closed.
    #[must_use]
    pub fn transform_for_canonical_face_position_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        position: usize,
        expected_face: FaceId,
    ) -> Option<IntervalRigidTransformV1> {
        if !std::ptr::eq(self.geometry, geometry) {
            return None;
        }
        self.registry
            .transform_for_canonical_face_position_v2(geometry, position, expected_face)
    }
}

impl CommonArticulationDynamicClosureBridgeV2 {
    /// Computes the finite retained-input and revalidation peak inventory
    /// without replaying the bridge. This is resource metadata only; it does
    /// not claim that the supplied schedule matches the bridge.
    pub fn checked_interval_transform_session_resources_v2(
        &self,
        schedule: &CanonicalCycleScheduleV1,
    ) -> Result<
        CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
        CommonArticulationDynamicClosureBridgeErrorV2,
    > {
        self.checked_interval_transform_session_resources_with_checkpoint_v2(
            schedule,
            self.parent_schedule_retained_cap_v2(),
            || Ok(()),
        )
    }

    /// Checkpointed retained-input and replay-peak inventory.
    pub fn checked_interval_transform_session_resources_with_checkpoint_v2(
        &self,
        schedule: &CanonicalCycleScheduleV1,
        max_schedule_retained_bytes: usize,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
    ) -> Result<
        CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
        CommonArticulationDynamicClosureBridgeErrorV2,
    > {
        checkpoint_bridge_v2(&mut checkpoint)?;
        let resources = checked_session_resources_with_checkpoint_v2(
            self,
            schedule,
            max_schedule_retained_bytes.min(self.parent_schedule_retained_cap_v2()),
            &mut checkpoint,
        )?;
        checkpoint_bridge_v2(&mut checkpoint)?;
        Ok(resources)
    }

    /// Revalidates this bridge against the complete live tuple and returns a
    /// sealed session for allocation-bounded interval transform observations.
    pub fn prepare_interval_transform_session_v2<'a>(
        &'a self,
        input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a>,
    ) -> Result<
        CommonArticulationDynamicClosureIntervalTransformSessionV2<'a>,
        CommonArticulationDynamicClosureBridgeErrorV2,
    > {
        self.prepare_interval_transform_session_with_checkpoint_v2(input, || Ok(()))
    }

    /// Checkpointed form of [`Self::prepare_interval_transform_session_v2`].
    pub fn prepare_interval_transform_session_with_checkpoint_v2<'a>(
        &'a self,
        input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
    ) -> Result<
        CommonArticulationDynamicClosureIntervalTransformSessionV2<'a>,
        CommonArticulationDynamicClosureBridgeErrorV2,
    > {
        checkpoint_bridge_v2(&mut checkpoint)?;
        let resources = checked_session_resources_with_checkpoint_v2(
            self,
            input.parent_schedule,
            self.parent_schedule_retained_cap_v2(),
            &mut checkpoint,
        )?;
        self.revalidate_with_checkpoint_v2(input, &mut checkpoint)?;
        checkpoint_bridge_v2(&mut checkpoint)?;
        Ok(CommonArticulationDynamicClosureIntervalTransformSessionV2 {
            bridge: self,
            input,
            resources,
        })
    }
}

impl<'a> CommonArticulationDynamicClosureIntervalTransformSessionV2<'a> {
    #[must_use]
    pub const fn resources(
        &self,
    ) -> CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2 {
        self.resources
    }

    /// Binding of the revalidated bridge. This is identity evidence only and
    /// is not an authority or a closure predicate.
    #[must_use]
    pub const fn bridge_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.bridge.binding_fingerprint_v2()
    }

    /// Internally evaluates one canonical dyadic leaf and builds only the
    /// spanning-tree transform observations justified by the already
    /// revalidated all-domain bridge theorem. The sealed workspace byte
    /// request must equal `schedule_workspace_bound.peak_bytes()`; caller
    /// policy ceilings belong to the preceding bound-validation seam.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_leaf_with_checkpoint_v2(
        &self,
        depth: u32,
        index: u64,
        schedule_limits: CycleScheduleLimitsV1,
        schedule_workspace_bound: CycleScheduleDyadicWorkspaceBoundV2,
        exact_schedule_workspace_bytes: usize,
        max_coverage_search_comparisons: usize,
        interval_transform_workspace_bound: &IntervalFaceTransformWorkspaceBoundV2,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
    ) -> Result<
        CommonArticulationDynamicClosureIntervalTransformLeafV2<'a>,
        CommonArticulationDynamicClosureIntervalTransformLeafErrorV2,
    > {
        checkpoint_leaf_v2(&mut checkpoint)?;
        if max_coverage_search_comparisons < self.resources.coverage_search_comparison_upper_bound
            || max_coverage_search_comparisons == usize::MAX
        {
            return Err(
                CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit,
            );
        }
        self.input
            .parent_schedule
            .validate_dyadic_workspace_request_with_checkpoint_v2(
                depth,
                schedule_limits,
                schedule_workspace_bound,
                exact_schedule_workspace_bytes,
                || {
                    checkpoint().map_err(|stop| match stop {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
                            CycleScheduleDyadicEvaluationStopV2::Cancelled
                        }
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
                            CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                        }
                    })
                },
            )
            .map_err(map_schedule_error_v2)?;
        interval_transform_workspace_bound
            .validate_for_input_with_checkpoint_v2(
                self.input.geometry,
                self.input.audit,
                self.input.parent_fixed_face,
                || {
                    checkpoint().map_err(|stop| match stop {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
                            DyadicIntervalClosureStopV1::Cancelled
                        }
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
                            DyadicIntervalClosureStopV1::DeadlineExceeded
                        }
                    })
                },
            )
            .map_err(map_registry_error_v2)?;
        let covered = parent_partition_covers_leaf_v2(
            self.bridge,
            depth,
            index,
            max_coverage_search_comparisons,
            &mut checkpoint,
        )?;
        if !covered {
            return Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive);
        }
        let evaluation = self
            .input
            .parent_schedule
            .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
                depth,
                index,
                schedule_limits,
                schedule_workspace_bound,
                exact_schedule_workspace_bytes,
                || {
                    checkpoint().map_err(|stop| match stop {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
                            CycleScheduleDyadicEvaluationStopV2::Cancelled
                        }
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
                            CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                        }
                    })
                },
            )
            .map_err(map_schedule_error_v2)?;
        let angle_box_capacity_bytes = evaluation.angle_box_capacity_bytes();
        let angle_boxes = evaluation.into_angle_boxes();
        checkpoint_leaf_v2(&mut checkpoint)?;
        let registry = self
            .input
            .geometry
            .prepare_spanning_interval_face_transform_registry_v2(
                self.input.audit,
                self.input.parent_fixed_face,
                &angle_boxes,
                self.input.closure_tolerance,
                interval_transform_workspace_bound,
                || {
                    checkpoint().map_err(|stop| match stop {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
                            DyadicIntervalClosureStopV1::Cancelled
                        }
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
                            DyadicIntervalClosureStopV1::DeadlineExceeded
                        }
                    })
                },
            )
            .map_err(map_registry_error_v2)?;
        let registry_resources = registry.resources();
        let leaf_wrapper_overhead_bytes = self.resources.leaf_wrapper_overhead_bytes;
        let retained_leaf_bytes = registry_resources
            .retained_registry_bytes()
            .checked_add(leaf_wrapper_overhead_bytes)
            .ok_or(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit)?;
        let registry_phase_peak_bytes = angle_box_capacity_bytes
            .checked_add(registry_resources.construction_peak_bytes())
            .ok_or(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit)?;
        let leaf_phase_peak_bytes = schedule_workspace_bound
            .peak_bytes()
            .max(registry_phase_peak_bytes)
            .max(retained_leaf_bytes);
        checkpoint_leaf_v2(&mut checkpoint)?;
        Ok(CommonArticulationDynamicClosureIntervalTransformLeafV2 {
            geometry: self.input.geometry,
            registry,
            resources: CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2 {
                schedule_workspace_upper_bound_bytes: schedule_workspace_bound.peak_bytes(),
                angle_box_capacity_bytes,
                registry_resources,
                leaf_wrapper_overhead_bytes,
                retained_leaf_bytes,
                leaf_phase_peak_bytes,
            },
        })
    }
}

fn checkpoint_leaf_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
) -> Result<(), CommonArticulationDynamicClosureIntervalTransformLeafErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::DeadlineExceeded
        }
    })
}

fn map_schedule_error_v2(
    error: CycleScheduleDyadicEvaluationErrorV2,
) -> CommonArticulationDynamicClosureIntervalTransformLeafErrorV2 {
    match error {
        CycleScheduleDyadicEvaluationErrorV2::Prepare(CycleSchedulePrepareErrorV1::AngleRange) => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive
        }
        CycleScheduleDyadicEvaluationErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::InvalidInput | CycleSchedulePrepareErrorV1::NonCanonical,
        ) => CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput,
        CycleScheduleDyadicEvaluationErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::ResourceLimit,
        )
        | CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit
        }
        CycleScheduleDyadicEvaluationErrorV2::Cancelled => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled
        }
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::DeadlineExceeded
        }
    }
}

fn map_schedule_resource_error_v2(
    error: CycleScheduleDyadicEvaluationErrorV2,
) -> CommonArticulationDynamicClosureBridgeErrorV2 {
    match error {
        CycleScheduleDyadicEvaluationErrorV2::Cancelled => {
            CommonArticulationDynamicClosureBridgeErrorV2::Cancelled
        }
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded
        }
        CycleScheduleDyadicEvaluationErrorV2::Prepare(_)
        | CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit => {
            CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit
        }
    }
}

fn map_registry_error_v2(
    error: IntervalFaceTransformWorkspaceErrorV2,
) -> CommonArticulationDynamicClosureIntervalTransformLeafErrorV2 {
    match error {
        IntervalFaceTransformWorkspaceErrorV2::InvalidInput => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput
        }
        IntervalFaceTransformWorkspaceErrorV2::ResourceLimit => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit
        }
        IntervalFaceTransformWorkspaceErrorV2::Unproven => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive
        }
        IntervalFaceTransformWorkspaceErrorV2::Cancelled => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled
        }
        IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::DeadlineExceeded
        }
    }
}
