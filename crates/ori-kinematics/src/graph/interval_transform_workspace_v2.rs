//! Workspace-bounded, observation-only interval face transforms.
//!
//! This wrapper reuses the vector-based closure traversal. It retains the
//! traversal's canonical pose vector directly, so no geometry clone, hash
//! table, angle-bit vector, or second transform allocation survives issuance.

use std::{fmt, mem::size_of};

use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};

use super::dyadic_workspace_v2::{
    IntervalAttemptErrorV2, IntervalClosureRequestV2, IntervalClosureVerificationModeV2,
    prove_interval_closure_with_workspace_v2,
};
use super::*;

mod types;
mod validation;

pub use types::{
    IntervalFaceTransformWorkspaceBoundV2, IntervalFaceTransformWorkspaceErrorV2,
    IntervalFaceTransformWorkspaceLimitsV2, IntervalFaceTransformWorkspaceResourcesV2,
};
use validation::*;

/// Observation-only interval transforms retained under a checked workspace
/// policy. This value grants no pose, collision, or simulation authority.
pub(crate) struct WorkspaceBoundedMaterialFaceTransformRegistryV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    fixed_face: FaceId,
    poses: Vec<Option<IntervalRigidTransformV1>>,
    input_binding: [u8; 32],
    tolerance_bits: u64,
    max_work: usize,
    resources: IntervalFaceTransformWorkspaceResourcesV2,
}

impl fmt::Debug for WorkspaceBoundedMaterialFaceTransformRegistryV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceBoundedMaterialFaceTransformRegistryV2")
            .field("resources", &self.resources)
            .finish_non_exhaustive()
    }
}

impl WorkspaceBoundedMaterialFaceTransformRegistryV2 {
    /// Checked and physically observed allocation inventory.
    #[must_use]
    pub(crate) const fn resources(&self) -> IntervalFaceTransformWorkspaceResourcesV2 {
        self.resources
    }

    /// Constant-time transform lookup for a caller already traversing the
    /// issuer's canonical face slice. The expected face prevents index drift.
    #[must_use]
    pub(crate) fn transform_for_canonical_face_position_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        position: usize,
        expected_face: FaceId,
    ) -> Option<IntervalRigidTransformV1> {
        if !self.issuer_geometry.matches(geometry)
            || geometry.face_ids().get(position).copied() != Some(expected_face)
        {
            return None;
        }
        self.poses.get(position).copied().flatten()
    }

    /// Allocation-free comparison with the complete live preparation input.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn matches_binding_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        max_work: usize,
    ) -> bool {
        self.matches_binding_with_checkpoint_v2(
            geometry,
            audit,
            fixed_face,
            angle_boxes,
            tolerance,
            max_work,
            || Ok(()),
        )
        .unwrap_or(false)
    }

    /// Checkpointed, allocation-free comparison with the complete live input.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn matches_binding_with_checkpoint_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        max_work: usize,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<bool, IntervalFaceTransformWorkspaceErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        let matches = self.issuer_geometry.matches(geometry)
            && self.fixed_face == fixed_face
            && self.tolerance_bits == tolerance.to_bits()
            && self.max_work == max_work
            && self.poses.len() == geometry.face_ids().len()
            && interval_face_transform_input_binding_v2(
                geometry,
                audit,
                fixed_face,
                angle_boxes,
                tolerance,
                max_work,
                &mut checkpoint,
            )
            .is_ok_and(|binding| binding == self.input_binding);
        checkpoint_v2(&mut checkpoint)?;
        Ok(matches)
    }
}

#[cfg(test)]
#[path = "interval_transform_workspace_v2/tests.rs"]
mod tests;

impl MaterialHingeGraphGeometry {
    /// Computes an allocation-free, representation-aware capacity inventory
    /// for the vector-based interval transform traversal.
    pub fn checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        limits: IntervalFaceTransformWorkspaceLimitsV2,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<IntervalFaceTransformWorkspaceBoundV2, IntervalFaceTransformWorkspaceErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        if [
            limits.max_work,
            limits.max_validation_work,
            limits.max_sort_comparisons,
            limits.max_workspace_bytes,
            limits.max_retained_bytes,
        ]
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit);
        }
        // Derive and enforce all cardinality-only hard bounds before walking
        // any caller-sized carrier.
        let face_count = self.face_ids().len();
        let hinge_count = self.hinges().len();
        let bit_length = usize::BITS as usize - hinge_count.leading_zeros() as usize;
        // Heap construction performs at most H/2 sifts and extraction fewer
        // than H sifts. Each level uses at most two key comparisons and has at
        // most `bit_length(H)` levels, hence 3*H*bit_length is an upper bound.
        let sort_comparison_upper_bound = hinge_count
            .checked_mul(bit_length)
            .and_then(|value| value.checked_mul(3))
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let validation_work_upper_bound = checked_validation_work_upper_bound_v2(
            face_count,
            hinge_count,
            audit.spanning_hinges().len(),
            audit.closure_hinges().len(),
        )
        .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let canonical_hinge_index_bytes = checked_vec_bytes_v2::<usize>(hinge_count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let adjacency_outer = checked_vec_bytes_v2::<Vec<(usize, usize, bool)>>(face_count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let adjacency_inner = audit
            .spanning_hinges()
            .len()
            .checked_mul(2)
            .and_then(checked_vec_bytes_v2::<(usize, usize, bool)>)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let degrees = checked_vec_bytes_v2::<usize>(face_count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let poses = checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(face_count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let queue = checked_vec_bytes_v2::<usize>(face_count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let interval_closure_bytes = adjacency_outer
            .checked_add(adjacency_inner)
            .and_then(|value| value.checked_add(degrees))
            .and_then(|value| value.checked_add(poses))
            .and_then(|value| value.checked_add(queue))
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let retained_registry_bytes = size_of::<WorkspaceBoundedMaterialFaceTransformRegistryV2>()
            .checked_add(poses)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let construction_peak_bytes = size_of::<WorkspaceBoundedMaterialFaceTransformRegistryV2>()
            .checked_add(canonical_hinge_index_bytes)
            .and_then(|value| value.checked_add(interval_closure_bytes))
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let checked_resources = IntervalFaceTransformWorkspaceResourcesV2 {
            validation_work_upper_bound,
            sort_comparison_upper_bound,
            canonical_hinge_index_bytes,
            interval_closure_bytes,
            retained_registry_bytes,
            construction_peak_bytes,
        };
        if validation_work_upper_bound > limits.max_validation_work
            || sort_comparison_upper_bound > limits.max_sort_comparisons
            || construction_peak_bytes > limits.max_workspace_bytes
            || retained_registry_bytes > limits.max_retained_bytes
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit);
        }
        let (validated_face_count, validated_hinge_count) =
            validate_canonical_audit_v2(self, audit, fixed_face, &mut checkpoint)?;
        if validated_face_count != face_count || validated_hinge_count != hinge_count {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        let audit_binding = audit_binding_with_checkpoint_v2(audit, &mut checkpoint)?;
        checkpoint_v2(&mut checkpoint)?;
        Ok(IntervalFaceTransformWorkspaceBoundV2 {
            issuer_geometry: self.instance_anchor_v1(),
            fixed_face,
            audit_binding,
            face_count,
            hinge_count,
            limits,
            checked_resources,
        })
    }

    /// Builds an observation-only registry using the checked bound from the
    /// same geometry instance. Every allocation is fallible and its physical
    /// capacity is checked before interval traversal continues.
    #[cfg(test)]
    pub(crate) fn prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        bound: &IntervalFaceTransformWorkspaceBoundV2,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<
        WorkspaceBoundedMaterialFaceTransformRegistryV2,
        IntervalFaceTransformWorkspaceErrorV2,
    > {
        self.prepare_interval_face_transform_registry_impl_v2(
            audit,
            fixed_face,
            angle_boxes,
            tolerance,
            bound,
            IntervalClosureVerificationModeV2::FullClosure,
            &mut checkpoint,
        )
    }

    /// Generates spanning-tree pose observations after a separately sealed
    /// all-domain closure session has revalidated the same live tuple. This is
    /// crate-private so no external caller can silently omit closure hinges.
    pub(crate) fn prepare_spanning_interval_face_transform_registry_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        bound: &IntervalFaceTransformWorkspaceBoundV2,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<
        WorkspaceBoundedMaterialFaceTransformRegistryV2,
        IntervalFaceTransformWorkspaceErrorV2,
    > {
        self.prepare_interval_face_transform_registry_impl_v2(
            audit,
            fixed_face,
            angle_boxes,
            tolerance,
            bound,
            IntervalClosureVerificationModeV2::SpanningObservation,
            &mut checkpoint,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_interval_face_transform_registry_impl_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        bound: &IntervalFaceTransformWorkspaceBoundV2,
        verification_mode: IntervalClosureVerificationModeV2,
        checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<
        WorkspaceBoundedMaterialFaceTransformRegistryV2,
        IntervalFaceTransformWorkspaceErrorV2,
    > {
        checkpoint_v2(checkpoint)?;
        bound.validate_for_input_with_checkpoint_v2(self, audit, fixed_face, &mut *checkpoint)?;
        if !tolerance.is_finite() || tolerance < 0.0 || angle_boxes.len() != bound.hinge_count {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }

        let mut canonical_hinge_indices = Vec::<usize>::new();
        canonical_hinge_indices
            .try_reserve_exact(bound.hinge_count)
            .map_err(|_| IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let canonical_hinge_index_bytes =
            checked_vec_bytes_v2::<usize>(canonical_hinge_indices.capacity())
                .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let shell_bytes = size_of::<WorkspaceBoundedMaterialFaceTransformRegistryV2>();
        if shell_bytes
            .checked_add(canonical_hinge_index_bytes)
            .is_none_or(|value| value > bound.limits.max_workspace_bytes)
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit);
        }
        for index in 0..bound.hinge_count {
            checkpoint_v2(checkpoint)?;
            canonical_hinge_indices.push(index);
        }
        checkpoint_heap_sort_by_key_v1(
            &mut canonical_hinge_indices,
            |index| self.hinges()[*index].edge().canonical_bytes(),
            checkpoint,
        )
        .map_err(|error| match error {
            CheckpointHeapSortErrorV1::ResourceLimit => {
                IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
            }
            CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::Cancelled) => {
                IntervalFaceTransformWorkspaceErrorV2::Cancelled
            }
            CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::DeadlineExceeded) => {
                IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded
            }
        })?;
        for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
            checkpoint_v2(checkpoint)?;
            let edge = self.hinges()[geometry_index].edge();
            let is_spanning = audit
                .spanning_hinges()
                .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
                .is_ok();
            let is_closure = audit
                .closure_hinges()
                .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
                .is_ok();
            if is_spanning == is_closure
                || angle_boxes.get(position).map(|(candidate, _)| *candidate) != Some(edge)
                || (position > 0
                    && self.hinges()[canonical_hinge_indices[position - 1]]
                        .edge()
                        .canonical_bytes()
                        >= edge.canonical_bytes())
            {
                return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
            }
        }
        let input_binding = interval_face_transform_input_binding_v2(
            self,
            audit,
            fixed_face,
            angle_boxes,
            tolerance,
            bound.limits.max_work,
            checkpoint,
        )?;
        let max_interval_closure_bytes = bound
            .limits
            .max_workspace_bytes
            .checked_sub(shell_bytes)
            .and_then(|value| value.checked_sub(canonical_hinge_index_bytes))
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let max_pose_capacity_bytes = bound
            .limits
            .max_retained_bytes
            .checked_sub(shell_bytes)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let success = prove_interval_closure_with_workspace_v2(
            IntervalClosureRequestV2 {
                geometry: self,
                audit,
                fixed_face,
                canonical_hinge_indices: &canonical_hinge_indices,
                angle_boxes,
                tolerance,
                max_work: bound.limits.max_work,
                max_workspace_bytes: max_interval_closure_bytes,
                max_pose_capacity_bytes,
                verification_mode,
            },
            checkpoint,
        )
        .map_err(map_interval_attempt_error_v2)?;
        let pose_capacity_bytes =
            checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(success.poses.capacity())
                .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let retained_registry_bytes = shell_bytes
            .checked_add(pose_capacity_bytes)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        let construction_peak_bytes = shell_bytes
            .checked_add(canonical_hinge_index_bytes)
            .and_then(|value| value.checked_add(success.physical_capacity_bytes))
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
        if retained_registry_bytes > bound.limits.max_retained_bytes
            || construction_peak_bytes > bound.limits.max_workspace_bytes
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit);
        }
        let resources = IntervalFaceTransformWorkspaceResourcesV2 {
            validation_work_upper_bound: bound.checked_resources.validation_work_upper_bound,
            sort_comparison_upper_bound: bound.checked_resources.sort_comparison_upper_bound,
            canonical_hinge_index_bytes,
            interval_closure_bytes: success.physical_capacity_bytes,
            retained_registry_bytes,
            construction_peak_bytes,
        };
        let registry = WorkspaceBoundedMaterialFaceTransformRegistryV2 {
            issuer_geometry: self.instance_anchor_v1(),
            fixed_face,
            poses: success.poses,
            input_binding,
            tolerance_bits: tolerance.to_bits(),
            max_work: bound.limits.max_work,
            resources,
        };
        if !registry.matches_binding_with_checkpoint_v2(
            self,
            audit,
            fixed_face,
            angle_boxes,
            tolerance,
            bound.limits.max_work,
            &mut *checkpoint,
        )? {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        checkpoint_v2(checkpoint)?;
        Ok(registry)
    }
}
