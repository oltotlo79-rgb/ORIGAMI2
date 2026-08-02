//! Public limits, opaque bounds, resources and errors for interval transforms.

use super::validation::{audit_binding_with_checkpoint_v2, checkpoint_v2};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntervalFaceTransformWorkspaceLimitsV2 {
    pub max_work: usize,
    pub max_validation_work: usize,
    pub max_sort_comparisons: usize,
    pub max_workspace_bytes: usize,
    pub max_retained_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntervalFaceTransformWorkspaceResourcesV2 {
    pub(super) validation_work_upper_bound: usize,
    pub(super) sort_comparison_upper_bound: usize,
    pub(super) canonical_hinge_index_bytes: usize,
    pub(super) interval_closure_bytes: usize,
    pub(super) retained_registry_bytes: usize,
    pub(super) construction_peak_bytes: usize,
}

impl IntervalFaceTransformWorkspaceResourcesV2 {
    #[must_use]
    pub const fn validation_work_upper_bound(self) -> usize {
        self.validation_work_upper_bound
    }

    #[must_use]
    pub const fn sort_comparison_upper_bound(self) -> usize {
        self.sort_comparison_upper_bound
    }

    #[must_use]
    pub const fn canonical_hinge_index_bytes(self) -> usize {
        self.canonical_hinge_index_bytes
    }

    #[must_use]
    pub const fn interval_closure_bytes(self) -> usize {
        self.interval_closure_bytes
    }

    #[must_use]
    pub const fn retained_registry_bytes(self) -> usize {
        self.retained_registry_bytes
    }

    #[must_use]
    pub const fn construction_peak_bytes(self) -> usize {
        self.construction_peak_bytes
    }
}

/// Opaque allocation-free preflight for one geometry/audit binding.
///
/// It is neither cloneable nor serializable and can only be used with the
/// same prepared geometry instance from which it was issued.
pub struct IntervalFaceTransformWorkspaceBoundV2 {
    pub(super) issuer_geometry: MaterialHingeGraphInstanceV1,
    pub(super) fixed_face: FaceId,
    pub(super) audit_binding: [u8; 32],
    pub(super) face_count: usize,
    pub(super) hinge_count: usize,
    pub(super) limits: IntervalFaceTransformWorkspaceLimitsV2,
    pub(super) checked_resources: IntervalFaceTransformWorkspaceResourcesV2,
}

impl fmt::Debug for IntervalFaceTransformWorkspaceBoundV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IntervalFaceTransformWorkspaceBoundV2")
            .field("face_count", &self.face_count)
            .field("hinge_count", &self.hinge_count)
            .field("checked_resources", &self.checked_resources)
            .finish_non_exhaustive()
    }
}

impl IntervalFaceTransformWorkspaceBoundV2 {
    #[must_use]
    pub const fn checked_resources(&self) -> IntervalFaceTransformWorkspaceResourcesV2 {
        self.checked_resources
    }

    pub(crate) fn validate_for_input_with_checkpoint_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<(), IntervalFaceTransformWorkspaceErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        if !self.issuer_geometry.matches(geometry)
            || self.fixed_face != fixed_face
            || self.face_count != geometry.face_ids().len()
            || self.hinge_count != geometry.hinges().len()
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        let audit_hinge_count = audit
            .spanning_hinges()
            .len()
            .checked_add(audit.closure_hinges().len())
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        if audit.faces().len() != self.face_count || audit_hinge_count != self.hinge_count {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        if [
            self.limits.max_work,
            self.limits.max_validation_work,
            self.limits.max_sort_comparisons,
            self.limits.max_workspace_bytes,
            self.limits.max_retained_bytes,
        ]
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
            || self.checked_resources.validation_work_upper_bound > self.limits.max_validation_work
            || self.checked_resources.sort_comparison_upper_bound > self.limits.max_sort_comparisons
            || self.checked_resources.construction_peak_bytes > self.limits.max_workspace_bytes
            || self.checked_resources.retained_registry_bytes > self.limits.max_retained_bytes
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit);
        }
        if audit_binding_with_checkpoint_v2(audit, &mut checkpoint)? != self.audit_binding {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        checkpoint_v2(&mut checkpoint)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalFaceTransformWorkspaceErrorV2 {
    InvalidInput,
    ResourceLimit,
    Unproven,
    Cancelled,
    DeadlineExceeded,
}
