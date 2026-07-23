//! Fail-closed bridge from authenticated effective-cut kinematics to future
//! static positive-thickness collision analysis.
//!
//! This module binds prerequisites only. It neither reconstructs the opaque
//! kinematics geometry nor claims that any face pair is collision-free.

use ori_kinematics::{EffectiveCutKinematicsDiagnosticV1, TreeKinematicsLimits};
use ori_topology::{EffectiveCutMaterialSnapshotDiagnosticV1, FaceExtractionInput};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1: &str =
    "effective_cut_static_thickness_prerequisite_v1";

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
            && kinematics.is_for(effective, input, kinematics_limits)
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
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1.as_bytes());
    hash.update(kinematics.fingerprint_v1());
    hash.update(thickness.to_bits().to_be_bytes());
    hash.update((kinematics.face_count() as u64).to_be_bytes());
    hash.update((kinematics.hinge_count() as u64).to_be_bytes());
    hash.update((pair_count as u64).to_be_bytes());
    hash.update((limits.max_face_pairs as u64).to_be_bytes());
    let fingerprint = hash.finalize().into();
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
