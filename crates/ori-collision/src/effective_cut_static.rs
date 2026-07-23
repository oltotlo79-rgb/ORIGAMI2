//! Fail-closed bridge from authenticated effective-cut kinematics to future
//! static positive-thickness collision analysis.
//!
//! This module binds prerequisites only. It neither reconstructs the opaque
//! kinematics geometry nor claims that any face pair is collision-free.

use ori_kinematics::{
    EffectiveCutKinematicsDiagnosticV1, EffectiveCutRetainedFacePairRegistryLimitsV1,
    EffectiveCutRetainedFacePairRegistryV1, TreeKinematicsLimits,
};
use ori_topology::{EffectiveCutMaterialSnapshotDiagnosticV1, FaceExtractionInput};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1: &str =
    "effective_cut_static_thickness_prerequisite_v1";
pub const EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1: &str =
    "effective_cut_static_pair_registry_bridge_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EffectiveCutStaticThicknessPrerequisiteErrorV1 {
    #[error("effective-cut kinematics binding is stale, foreign, or unsupported")]
    InvalidBinding,
    #[error("paper thickness must be finite and strictly positive")]
    InvalidThickness,
    #[error("static face-pair work exceeds the configured resource limit")]
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCutStaticThicknessLimitsV1 {
    pub max_face_pairs: usize,
}

impl Default for EffectiveCutStaticThicknessLimitsV1 {
    fn default() -> Self {
        Self {
            max_face_pairs: 1_000_000,
        }
    }
}

/// Opaque, non-authoritative prerequisite for future static collision work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
    kinematics_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    thickness_bits: u64,
    face_count: usize,
    hinge_count: usize,
    pair_count: usize,
    kinematics_limits: TreeKinematicsLimits,
    limits: EffectiveCutStaticThicknessLimitsV1,
}

impl EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub const fn face_count(&self) -> usize {
        self.face_count
    }
    #[must_use]
    pub const fn hinge_count(&self) -> usize {
        self.hinge_count
    }
    #[must_use]
    /// Planned unordered-pair work cardinality. This is not a pair-evidence
    /// registry and proves neither pair coverage nor separation.
    pub const fn planned_unordered_face_pair_count(&self) -> usize {
        self.pair_count
    }
    #[must_use]
    pub const fn observes_source_flat_convention_only(&self) -> bool {
        true
    }
    #[must_use]
    pub const fn paper_thickness_mm(&self) -> f64 {
        f64::from_bits(self.thickness_bits)
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        kinematics: &EffectiveCutKinematicsDiagnosticV1,
        effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
        input: FaceExtractionInput<'_>,
        kinematics_limits: TreeKinematicsLimits,
        limits: EffectiveCutStaticThicknessLimitsV1,
    ) -> bool {
        self.limits == limits
            && self.kinematics_limits == kinematics_limits
            && self.kinematics_fingerprint == kinematics.fingerprint_v1()
            && input.paper.thickness_mm.to_bits() == self.thickness_bits
            && prepare_effective_cut_static_thickness_prerequisite_v1(
                kinematics,
                effective,
                input,
                kinematics_limits,
                limits,
            )
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

/// Opaque binding of a static-thickness prerequisite to the complete retained
/// face-pair registry. It performs no SAT or pair classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCutStaticPairRegistryBridgeV1 {
    prerequisite_fingerprint: [u8; 32],
    registry_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    pair_count: usize,
    kinematics_limits: TreeKinematicsLimits,
    prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
    registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
}

impl EffectiveCutStaticPairRegistryBridgeV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub const fn pair_count(&self) -> usize {
        self.pair_count
    }
    #[must_use]
    pub const fn authorizes_pair_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn is_for(
        &self,
        prerequisite: &EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
        registry: &EffectiveCutRetainedFacePairRegistryV1,
        kinematics: &EffectiveCutKinematicsDiagnosticV1,
        effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
        input: FaceExtractionInput<'_>,
        kinematics_limits: TreeKinematicsLimits,
        prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
        registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
    ) -> bool {
        self.prerequisite_fingerprint == prerequisite.fingerprint_v1()
            && self.registry_fingerprint == registry.fingerprint_v1()
            && self.kinematics_limits == kinematics_limits
            && self.prerequisite_limits == prerequisite_limits
            && self.registry_limits == registry_limits
            && prepare_effective_cut_static_pair_registry_bridge_v1(
                prerequisite,
                registry,
                kinematics,
                effective,
                input,
                kinematics_limits,
                prerequisite_limits,
                registry_limits,
            )
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_effective_cut_static_pair_registry_bridge_v1(
    prerequisite: &EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
    registry: &EffectiveCutRetainedFacePairRegistryV1,
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
    input: FaceExtractionInput<'_>,
    kinematics_limits: TreeKinematicsLimits,
    prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
    registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
) -> Result<EffectiveCutStaticPairRegistryBridgeV1, EffectiveCutStaticThicknessPrerequisiteErrorV1>
{
    if prerequisite_limits.max_face_pairs != registry_limits.max_pairs
        || prerequisite.kinematics_fingerprint != kinematics.fingerprint_v1()
        || prerequisite.kinematics_limits != kinematics_limits
        || prerequisite.limits != prerequisite_limits
        || prerequisite.thickness_bits != input.paper.thickness_mm.to_bits()
        || prerequisite.face_count != kinematics.face_count()
        || prerequisite.hinge_count != kinematics.hinge_count()
        || prerequisite.planned_unordered_face_pair_count() != registry.pair_count()
        || prerequisite.fingerprint
            != static_prerequisite_fingerprint_v1(
                kinematics,
                input.paper.thickness_mm,
                prerequisite.planned_unordered_face_pair_count(),
                prerequisite_limits,
            )
        || !registry.is_for(
            kinematics,
            effective,
            input,
            kinematics_limits,
            registry_limits,
        )
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let pair_count = registry.pair_count();
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1.as_bytes());
    hash.update(prerequisite.fingerprint_v1());
    hash.update(registry.fingerprint_v1());
    hash.update(kinematics.fingerprint_v1());
    hash.update(effective.fingerprint_v1());
    hash.update(prerequisite.thickness_bits.to_be_bytes());
    hash.update((prerequisite.face_count as u64).to_be_bytes());
    hash.update((prerequisite.hinge_count as u64).to_be_bytes());
    hash.update((pair_count as u64).to_be_bytes());
    hash.update((registry.shared_hinge_membership_count() as u64).to_be_bytes());
    for value in [
        kinematics_limits.max_source_vertices,
        kinematics_limits.max_source_edges,
        kinematics_limits.max_paper_boundary_vertices,
        kinematics_limits.max_faces,
        kinematics_limits.max_edge_incidences,
        kinematics_limits.max_hinges,
        kinematics_limits.max_face_boundary_vertices,
        kinematics_limits.max_adjacency_entries,
        prerequisite_limits.max_face_pairs,
        registry_limits.max_pairs,
        registry_limits.max_shared_hinge_memberships,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    Ok(EffectiveCutStaticPairRegistryBridgeV1 {
        prerequisite_fingerprint: prerequisite.fingerprint_v1(),
        registry_fingerprint: registry.fingerprint_v1(),
        fingerprint: hash.finalize().into(),
        pair_count,
        kinematics_limits,
        prerequisite_limits,
        registry_limits,
    })
}

pub fn prepare_effective_cut_static_thickness_prerequisite_v1(
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
    input: FaceExtractionInput<'_>,
    kinematics_limits: TreeKinematicsLimits,
    limits: EffectiveCutStaticThicknessLimitsV1,
) -> Result<
    EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
    EffectiveCutStaticThicknessPrerequisiteErrorV1,
> {
    let thickness = input.paper.thickness_mm;
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidThickness);
    }
    if !kinematics.is_for(effective, input, kinematics_limits) {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let pair_count = kinematics
        .face_count()
        .checked_sub(1)
        .and_then(|less| kinematics.face_count().checked_mul(less))
        .and_then(|twice| twice.checked_div(2))
        .filter(|count| *count <= limits.max_face_pairs)
        .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    let fingerprint = static_prerequisite_fingerprint_v1(kinematics, thickness, pair_count, limits);
    Ok(EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
        kinematics_fingerprint: kinematics.fingerprint_v1(),
        fingerprint,
        thickness_bits: thickness.to_bits(),
        face_count: kinematics.face_count(),
        hinge_count: kinematics.hinge_count(),
        pair_count,
        kinematics_limits,
        limits,
    })
}

fn static_prerequisite_fingerprint_v1(
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    thickness: f64,
    pair_count: usize,
    limits: EffectiveCutStaticThicknessLimitsV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1.as_bytes());
    hash.update(kinematics.fingerprint_v1());
    hash.update(thickness.to_bits().to_be_bytes());
    hash.update((kinematics.face_count() as u64).to_be_bytes());
    hash.update((kinematics.hinge_count() as u64).to_be_bytes());
    hash.update((pair_count as u64).to_be_bytes());
    hash.update((limits.max_face_pairs as u64).to_be_bytes());
    hash.finalize().into()
}
